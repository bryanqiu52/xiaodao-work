// 番茄钟的运行时：**模块级单例**，状态与计时不属于任何组件。
//
// **为什么不能挂在 FocusPane 里**：主界面切换工具会把那个组件卸载，
// 挂在组件里的 interval 和 state 一起消失 —— 表现是"切到别的工具，番茄钟就不跑了"。
// 番茄钟的语义本来就是"后台在计，到点了通知你"，所以它必须活在组件之外：
// 状态、interval、事件订阅都放模块级，组件只负责把状态画出来。
//
// 到点检测同样是两路：前端 250ms interval（前台时 UI 平滑）+ Rust `focus-tick`
// （15 秒一拍，不受 WebView 节流影响 —— 窗口藏托盘、切到别的工具都靠它），
// 两路都调同一个 checkDue，推进一次之后状态就不再是 running，重复触发天然幂等。
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { isTauri, logToBackend, tauriApi } from '../api/tauri'
import {
  advance,
  focusOptions,
  idleState,
  isDue,
  pause,
  progress,
  remainingMs,
  resume,
  skipPhase,
  startPhase,
  type FocusState,
} from '../core/focus'
import { configStore, saveConfig } from '../stores/config'
import { todayStr } from './useReminder'
import { addRecord } from './useFocusLog'
import { playChime } from '../utils/chime'
import type { TodoItem } from '../core/types'

const o = () => focusOptions(configStore.cfg)

export const focusState = ref(idleState('work', o(), 0))
export const focusNow = ref(Date.now())

// ── 挂在哪条待办上 ────────────────────────────────────────────────────────
//
// **运行态，不落盘** —— 跟番茄钟本身同一个口径（重启回到待开始）。
// 一个跨重启还留着的"正在专注某条待办"只会让人困惑：那条到底算不算在专注？
//
// 记的是 id + 标题两样：id 用来统计，标题用来写进记录（冗余存一份，
// 待办以后改名或删掉了，历史统计里还看得出当时干的是什么）。

export const focusTaskId = ref('')
export const focusTaskTitle = ref('')

/** 换 / 取消绑定。传 null = 取消（标题也一起清掉，别留半截） */
export function setFocusTask(item: Pick<TodoItem, 'id' | 'title'> | null): void {
  focusTaskId.value = item?.id ?? ''
  focusTaskTitle.value = item?.title ?? ''
}

/** 本地时刻 `YYYY-MM-DDTHH:mm:ss`。不用 `toISOString()` —— 那是 UTC，读起来要脑补 8 小时 */
function localStamp(): string {
  const d = new Date()
  const p = (n: number): string => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

/** 记录 id：`f` + 时间戳36进制 + 随机尾巴，跟待办的 id 生成方式同一个思路 */
function nextRecordId(): string {
  return `f${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

/**
 * 一个专注阶段正常结束 → 记一笔。
 *
 * **时长取 `state.durationMs`**（那一段开始时定下的总时长），不是现读配置 ——
 * 跑着的时候用户把 25 分钟改成 60 分钟，配置给出的是 60，
 * 而刚刚过去的其实是 25 分钟的一段，记 60 就是凭空多算了。
 *
 * 中途跳过（`skipPhase`）**不记** —— 与"跳过的专注不计轮数"同一个口径。
 */
async function recordFocus(durationMs: number): Promise<void> {
  const minutes = Math.max(1, Math.round(durationMs / 60_000))
  const err = await addRecord({
    id: nextRecordId(),
    at: localStamp(),
    date: todayStr(),
    minutes,
    todoId: focusTaskId.value,
    todoTitle: focusTaskTitle.value,
  })
  if (err !== null) {
    // 失败必须留痕：用户只会看到"番茄数了、统计里却没有"，分不清是坏了还是没到点
    logToBackend('warn', `专注记录没写进去：${err}`)
  }
}

// 设置改了时长：空闲态跟着重算；跑着的不动 —— 改时长影响的是下一个阶段
watch(
  () => [
    configStore.cfg.focus_work_min,
    configStore.cfg.focus_short_min,
    configStore.cfg.focus_long_min,
    configStore.cfg.focus_rounds,
  ],
  () => {
    if (focusState.value.run === 'idle') {
      focusState.value = idleState(focusState.value.phase, o(), focusState.value.round)
    }
  },
)

export const focusRemaining = computed(() => remainingMs(focusState.value, focusNow.value))
export const focusProgress = computed(() => progress(focusState.value, focusNow.value))
export const focusTodayCount = computed(() =>
  configStore.cfg.focus_done_day === todayStr() ? configStore.cfg.focus_done_count : 0,
)

/**
 * **换状态必须连 focusNow 一起换。**
 * focusNow 是节拍器推进的"上一拍"时间戳，最多旧 250ms；只换 state 不换 now，
 * 剩余时间就变成 `时长 + 这个旧差值`，配上界面 `Math.ceil` 的显示，
 * 表现就是"刚点开始先跳 25:01，再落回 25:00"——凭空多出来的一秒。
 */
function commit(s: FocusState, now: number): void {
  focusState.value = s
  focusNow.value = now
}

/** 到点处理：推进状态机 → 专注完成则计数 → 响铃 → 发系统通知 */
async function settle(): Promise<void> {
  const now = Date.now()
  // 先留一份推进前的状态：那上面有这一段**开始时定下的**总时长，
  // 记专注记录要用它（见 recordFocus 的说明）
  const before = focusState.value
  const out = advance(before, o(), now, configStore.cfg.focus_auto_continue)
  const completedWork = out.completedWork
  commit(out.next, now)
  if (completedWork) {
    await bumpToday()
    await recordFocus(before.durationMs)
  }
  if (configStore.cfg.focus_sound) playChime()
  const err = await tauriApi.notify(out.notifyTitle, out.notifyBody)
  if (err !== null) {
    // 通知发不出去必须留痕：用户只看到铃响没通知，分不清是系统拦了还是坏了
    logToBackend('warn', `番茄钟通知没发出去：${err}`)
  }
}

function checkDue(): void {
  if (isDue(focusState.value, focusNow.value)) void settle()
}

/** 今日完成数 +1：跨天清零（跟 remind_day 同一个口径，只记当天不留历史） */
async function bumpToday(): Promise<void> {
  const today = todayStr()
  const same = configStore.cfg.focus_done_day === today
  await saveConfig({
    focus_done_day: today,
    focus_done_count: (same ? configStore.cfg.focus_done_count : 0) + 1,
  })
}

export function focusPrimary(): void {
  const now = Date.now()
  const s = focusState.value
  if (s.run === 'idle') commit(startPhase(s.phase, o(), s.round, now), now)
  else if (s.run === 'running') commit(pause(s, now), now)
  else commit(resume(s, now), now)
}

/** 跳过：进入下一阶段，不算完成（跳过的专注不计轮数） */
export function focusSkip(): void {
  const now = Date.now()
  commit(skipPhase(focusState.value, o(), now), now)
}

/** 重置：回到专注待开始，轮数也清零 */
export function focusReset(): void {
  const now = Date.now()
  commit(idleState('work', o(), 0), now)
}

// ── 计时与订阅（幂等，可重复调用）────────────────────────────────────────

let uiTimer: number | undefined
let unlisten: (() => void) | undefined

function tick(): void {
  focusNow.value = Date.now()
  checkDue()
}

/**
 * 启动番茄钟的后台计时。**幂等**，重复调用不会起第二个 interval。
 * 由 App.vue 在挂载时调一次 —— 不依赖番茄钟界面是否打开。
 */
export function startFocus(): void {
  if (uiTimer === undefined) uiTimer = window.setInterval(tick, 250)
  if (isTauri() && unlisten === undefined) {
    void listen('focus-tick', () => {
      focusNow.value = Date.now()
      checkDue()
    })
      .then((fn) => {
        unlisten = fn
      })
      .catch((e) => logToBackend('error', `订阅 focus-tick 失败：${String(e)}`))
  }
}

/** App 卸载才停 —— 番茄钟界面关闭时不该停 */
export function stopFocus(): void {
  if (uiTimer !== undefined) {
    window.clearInterval(uiTimer)
    uiTimer = undefined
  }
  unlisten?.()
  unlisten = undefined
}

/** 界面里可选调用：切出番茄页时不需要做任何事，计时照跑 */
export function useFocusTimer(): void {
  onBeforeUnmount(() => {
    /* 刻意什么都不做：状态与计时是模块级的，不随组件走 */
  })
}
