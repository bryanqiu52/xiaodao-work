// 番茄闹钟的状态机：纯函数，不碰 DOM 也不碰 invoke。
//
// **计时用「结束时间戳」而不是递减计数**：WebView 被隐藏/节流后 setInterval
// 可能好几秒才跳一次 —— 剩余时间按 `deadline - now` 算，恢复时一步到位，
// 不会出现"藏了十分钟回来还剩九分钟"。
// 到点检测由两路触发：前端 250ms interval（前台、UI 平滑）+ Rust `focus-tick`
// （15s 一拍，不受 WebView 节流影响，窗口藏进托盘也不会漏）。
//
// 运行态**不落盘**：番茄钟是"当下"的事，重启后回到干净的待开始状态，
// 跨重启恢复一个旧倒计时只会让人困惑。唯一持久化的是「今日完成的番茄数」。

import type { AppConfig } from './config'

export type FocusPhase = 'work' | 'short' | 'long'
export type FocusRunState = 'idle' | 'running' | 'paused'

export interface FocusState {
  phase: FocusPhase
  run: FocusRunState
  /** 本阶段总时长（ms） */
  durationMs: number
  /** 运行中的截止时间戳；idle / paused 下无意义 */
  deadline: number
  /** 暂停时冻结的剩余毫秒 */
  pausedRemainingMs: number
  /** 本循环已完成的专注轮数 */
  round: number
}

/** 番茄钟需要的配置切片 —— 纯函数不吃大配置对象，字段从 AppConfig 抽 */
export interface FocusOptions {
  workMin: number
  shortMin: number
  longMin: number
  /** 长休前的专注轮数（默认 4：专注×4 → 长休） */
  rounds: number
}

export function focusOptions(cfg: AppConfig): FocusOptions {
  return {
    workMin: cfg.focus_work_min,
    shortMin: cfg.focus_short_min,
    longMin: cfg.focus_long_min,
    rounds: Math.max(1, cfg.focus_rounds),
  }
}

const MIN_MS = 60_000

export function phaseDurationMs(phase: FocusPhase, o: FocusOptions): number {
  const min = phase === 'work' ? o.workMin : phase === 'short' ? o.shortMin : o.longMin
  return Math.max(1, min) * MIN_MS
}

/** 停在某个阶段上的空闲态（不计时）。设置改了时长，空闲态的时长跟着重算 */
export function idleState(phase: FocusPhase, o: FocusOptions, round: number): FocusState {
  return {
    phase,
    run: 'idle',
    durationMs: phaseDurationMs(phase, o),
    deadline: 0,
    pausedRemainingMs: 0,
    round,
  }
}

export function startPhase(
  phase: FocusPhase,
  o: FocusOptions,
  round: number,
  now: number,
): FocusState {
  const durationMs = phaseDurationMs(phase, o)
  return {
    phase,
    run: 'running',
    durationMs,
    deadline: now + durationMs,
    pausedRemainingMs: 0,
    round,
  }
}

export function remainingMs(s: FocusState, now: number): number {
  if (s.run === 'running') return Math.max(s.deadline - now, 0)
  if (s.run === 'paused') return s.pausedRemainingMs
  return s.durationMs
}

export function isDue(s: FocusState, now: number): boolean {
  return s.run === 'running' && s.deadline <= now
}

/** 已流逝比例（0–1），进度环用 */
export function progress(s: FocusState, now: number): number {
  if (s.run === 'idle') return 0
  const remain = remainingMs(s, now)
  return Math.min(1, Math.max(0, 1 - remain / Math.max(s.durationMs, 1)))
}

export function pause(s: FocusState, now: number): FocusState {
  if (s.run !== 'running') return s
  return { ...s, run: 'paused', pausedRemainingMs: Math.max(s.deadline - now, 0) }
}

export function resume(s: FocusState, now: number): FocusState {
  if (s.run !== 'paused') return s
  return { ...s, run: 'running', deadline: now + s.pausedRemainingMs, pausedRemainingMs: 0 }
}

/** 专注做完后接哪个阶段：第 rounds 轮接长休，其余接短休；休息完回专注 */
export function nextPhase(phase: FocusPhase, finishedRounds: number, o: FocusOptions): FocusPhase {
  if (phase !== 'work') return 'work'
  return finishedRounds > 0 && finishedRounds % o.rounds === 0 ? 'long' : 'short'
}

export interface AdvanceOutcome {
  next: FocusState
  /** 完成的是不是专注（只有专注完成才计数，通知文案也不同） */
  completedWork: boolean
  notifyTitle: string
  notifyBody: string
}

/**
 * 阶段到点，推进到下一个。
 * **调用方保证已到点**（先用 isDue 判过）；这里不判时间，只负责"这一步走到哪"。
 */
export function advance(
  s: FocusState,
  o: FocusOptions,
  now: number,
  autoContinue: boolean,
): AdvanceOutcome {
  const completedWork = s.phase === 'work'
  const newRound = completedWork ? s.round + 1 : s.round
  const next = nextPhase(s.phase, newRound, o)
  const nextState = autoContinue
    ? startPhase(next, o, newRound, now)
    : idleState(next, o, newRound)
  return {
    next: nextState,
    completedWork,
    notifyTitle: completedWork ? '番茄完成' : '休息结束',
    notifyBody: completedWork
      ? `专注 ${Math.round(s.durationMs / MIN_MS)} 分钟完成，${next === 'long' ? '该长休了' : '休息一下'}`
      : '休息得差不多了，开始下一个番茄',
  }
}

/**
 * 跳过当前阶段：直接进入下一个，**不算完成、不发通知** ——
 * 和市面主流番茄钟一致（跳过的专注不计入轮数）。
 */
export function skipPhase(s: FocusState, o: FocusOptions, now: number): FocusState {
  const next = nextPhase(s.phase, s.round, o)
  return idleState(next, o, s.round)
}
