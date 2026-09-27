// 到期提醒的判定：挑出"该提醒谁"、拼通知文案。
//
// **纯函数，不碰 DOM 也不碰 invoke**：判定是业务逻辑，值得能单独测。
// 定时由 Rust 那边的 `reminder.rs` 每半小时喊一声，发通知由 `useReminder` 负责，
// 这里只管「该提醒哪些」和「通知上写什么」。

import { dueDay } from './view'
import type { TodoItem } from './types'

export interface RemindInput {
  items: readonly TodoItem[]
  /** 今天（`YYYY-MM-DD`，本地时区） */
  today: string
  /** 提前几天提醒（0 = 当天才提醒） */
  advanceDays: number
  /** 今天已经提醒过的条目 id */
  doneToday: readonly string[]
  /** 被「稍后再说」推迟过的条目 id */
  snoozed: readonly string[]
  /** 推迟到什么时候（毫秒时间戳；过去的时间 = 推迟期已过，重新参与提醒） */
  snoozeUntil: number
  /** 现在几点（毫秒）。显式传进来是为了让判定可测 */
  now: number
}

export interface RemindResult {
  /** 这次该弹的条目（已排除今天提醒过的） */
  due: TodoItem[]
  /** 提醒之后要把这些 id 记进 `remind_done` */
  markDone: string[]
}

/** 把 `YYYY-MM-DD` 往后推 n 天（n 可以为负） */
export function plusDays(day: string, n: number): string {
  const [y, m, d] = day.split('-').map(Number)
  const dt = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1)
  dt.setDate(dt.getDate() + n)
  const yy = dt.getFullYear()
  const mm = String(dt.getMonth() + 1).padStart(2, '0')
  const dd = String(dt.getDate()).padStart(2, '0')
  return `${yy}-${mm}-${dd}`
}

/** 两个 `YYYY-MM-DD` 相隔几天（后者减前者，负数表示前者更早） */
export function diffDays(from: string, to: string): number {
  const a = new Date(`${from}T00:00:00`).getTime()
  const b = new Date(`${to}T00:00:00`).getTime()
  return Math.round((b - a) / 86400000)
}

/**
 * 挑出该提醒的条目。
 *
 * 四个条件缺一不可：**未删除、未完成、有期限、到期日 ≤ 今天 + 提前量**，
 * 且今天还没提醒过它。
 *
 * 日期比较直接用字符串比（`YYYY-MM-DD` 的字典序就是时间序），不用转 Date ——
 * 少一次时区转换，就少一个跨时区/夏令时出错的地方。
 */
export function pickDue(input: RemindInput): RemindResult {
  const deadline = plusDays(input.today, input.advanceDays)
  const already = new Set(input.doneToday)
  /**
   * 「稍后再说」和「今天不再提醒」的区别就在这一行。
   *
   * 被推迟的条目在 snooze 那一刻就**从 `remind_done` 里摘掉了**（见 useReminder），
   * 所以推迟期一过它自然会重新进入判定；推迟期没到就先跳过。
   * 这两种语义要分开：`remind_done` 管"今天别再提"，snooze 管"过会儿再提"。
   */
  const snoozeActive = input.snoozeUntil > input.now
  const snoozed = new Set(input.snoozed)

  const due: TodoItem[] = []
  for (const it of input.items) {
    if (it.deletedAt !== null) continue // 软删除的不提醒
    if (it.status === 'done') continue // 已完成的不提醒
    const day = dueDay(it)
    if (day === '') continue // 没设期限的不管
    if (day > deadline) continue // 还没到该提醒的时候
    if (snoozeActive && snoozed.has(it.id)) continue // 说了稍后，还没到点
    if (already.has(it.id)) continue // 今天提醒过了
    due.push(it)
  }
  return { due, markDone: due.map((it) => it.id) }
}

/** 单条时的说法：今天到期 / 已过期 N 天 / 某天到期 */
function lineOf(item: TodoItem, today: string): string {
  const day = dueDay(item)
  if (day < today) return `${item.title}（已过期 ${diffDays(day, today)} 天）`
  if (day === today) return `${item.title}（今天到期）`
  return `${item.title}（${day} 到期）`
}

/**
 * 拼通知文案。**多条合并成一条** —— 十条待办到期就弹十条通知的话，
 * 用户第一件事是关通知，第二件事是把这个功能关掉。
 */
export function notifyText(
  due: readonly TodoItem[],
  today: string,
): { title: string; body: string } {
  const first = due[0]
  if (first === undefined) return { title: '', body: '' }

  if (due.length === 1) {
    return { title: '待办到期提醒', body: lineOf(first, today) }
  }
  // 正文最多列 5 条：系统通知区就那么宽，写长了被截断反而看不出重点
  const shown = due
    .slice(0, 5)
    .map((it) => `· ${lineOf(it, today)}`)
    .join('\n')
  const more = due.length > 5 ? `\n…等共 ${due.length} 条` : ''
  return { title: `有 ${due.length} 条待办到期了`, body: `${shown}${more}` }
}
