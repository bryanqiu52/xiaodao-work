// 到期提醒的装配：订阅 Rust 的 tick → 判定 → 发系统通知 → 记下"今天提醒过谁"。
//
// 判定逻辑在 `core/remind.ts`（纯函数），这里只管接线和副作用。
// 定时为什么在 Rust 而不在这里：WebView 隐藏时 `setInterval` 会被节流，
// 而"关掉窗口藏托盘"正是本应用最常见的用法（详见 `src-tauri/src/reminder.rs`）。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { logToBackend, tauriApi } from '../api/tauri'
import { notifyText, pickDue } from '../core/remind'
import { configStore, saveConfig } from '../stores/config'
import { todoStore } from '../stores/todo'

/**
 * 上一次发通知的结果。null = 成功（或还没发过）；字符串 = 失败原因。
 *
 * **导出给设置页看**：提醒发不出去时用户只会以为"还没到期"——
 * 这是提醒类功能里最糟的一种静默失败，所以失败原因必须有个看得见的地方。
 */
export const lastNotifyError = ref<string | null>(null)

/** 今天的日期串（本地时区）—— 用户说的"今天"是本地今天，不是 UTC 的今天 */
export function todayStr(): string {
  const d = new Date()
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  return `${d.getFullYear()}-${mm}-${dd}`
}

let unlisten: (() => void) | undefined

/**
 * 查一次：该提醒就发通知，并记下今天提醒过谁。返回这次提醒了几条。
 * 单独导出是为了让设置页能"立即检查"，也方便验证这条链路通不通。
 */
export async function checkRemindNow(): Promise<number> {
  if (!configStore.loaded) return 0 // 配置还没读回来，先不判
  if (!configStore.cfg.remind_enabled) return 0

  const today = todayStr()
  // 跨天了就把"今天提醒过谁"清空。**只记当天、不留历史** —— 留历史会无限增长
  const sameDay = configStore.cfg.remind_day === today
  const doneToday = sameDay ? configStore.cfg.remind_done : []

  // 「稍后再说」：推迟期没过就让这些条目先歇着。
  // 推迟期一过就顺手把这两个字段清掉 —— 不清也不影响判定（时间戳已经过期了），
  // 但会一直留着一串没用的 id。
  const until = configStore.cfg.remind_snooze_until
  const snoozeActive = until > Date.now()
  if (!snoozeActive && (until !== 0 || configStore.cfg.remind_snoozed.length > 0)) {
    await saveConfig({ remind_snooze_until: 0, remind_snoozed: [] })
  }

  const { due, markDone } = pickDue({
    items: todoStore.items,
    today,
    advanceDays: configStore.cfg.remind_advance_days,
    doneToday,
    snoozed: snoozeActive ? configStore.cfg.remind_snoozed : [],
    snoozeUntil: until,
    now: Date.now(),
  })

  if (due.length === 0) {
    // 跨天且今天没什么要提醒的：也把日期记上，省得以后每次都要重算"跨没跨天"
    if (!sameDay) await saveConfig({ remind_day: today, remind_done: [] })
    return 0
  }

  const { title, body } = notifyText(due, today)
  const err = await tauriApi.notify(title, body)
  if (err !== null) {
    // 提醒没发出去必须留痕：静默失败的提醒比没有提醒更糟 —— 用户以为自己没到期。
    // 同时把原因挂到 lastNotifyError 上，设置页「立即检查」那行能直接看到
    lastNotifyError.value = err
    logToBackend('warn', `到期提醒没发出去：${err}`)
    return 0
  }
  lastNotifyError.value = null
  await saveConfig({ remind_day: today, remind_done: [...doneToday, ...markDone] })
  logToBackend('info', `到期提醒已发，共 ${due.length} 条`)
  return due.length
}

/** 「稍后再说」往后推多久 */
export const SNOOZE_HOURS = 4

/**
 * 把一条待办「稍后再说」。
 *
 * **必须同时做两件事，少一件就不成立**：
 *   1. 把它从 `remind_done` 里摘出来 —— 它已经被标成"今天提醒过了"，
 *      不摘的话推迟期一到也捞不回来，就退化成"今天不再提醒"了；
 *   2. 记下推迟截止时间，让 `pickDue` 在这之前跳过它。
 * 只做 1 会立刻再弹一次；只做 2 则永远捞不回来。
 */
export async function snoozeItem(id: string): Promise<void> {
  const today = todayStr()
  const sameDay = configStore.cfg.remind_day === today
  const done = (sameDay ? configStore.cfg.remind_done : []).filter((x) => x !== id)
  await saveConfig({
    remind_day: today,
    remind_done: done,
    remind_snoozed: [...new Set([...configStore.cfg.remind_snoozed, id])],
    remind_snooze_until: Date.now() + SNOOZE_HOURS * 3600_000,
  })
  logToBackend('info', `已把 ${id} 推后 ${SNOOZE_HOURS} 小时再提醒`)
}

export function useReminder(): void {
  onMounted(async () => {
    try {
      unlisten = await listen('reminder-tick', () => {
        void checkRemindNow()
      })
      logToBackend('info', '已订阅 reminder-tick')
    } catch (e) {
      logToBackend('error', `订阅 reminder-tick 失败：${String(e)}`)
    }
    // 启动时先自己查一次：Rust 那边首次 tick 要等 15 秒，之后再等半小时 ——
    // 开机那天到期的事，不该拖到半小时后才提醒
    void checkRemindNow()
  })
  onBeforeUnmount(() => unlisten?.())
}
