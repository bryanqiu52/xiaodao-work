// 专注记录的读取与缓存：番茄页（"这条累计几个番茄"）和回顾页（"这段时间专注了多久"）
// 共用这一份，不去各读一次文件 —— 两处各读一次迟早出现"两个页面数字不一样"。
//
// 与 `useFocus.ts` 的分工：那边管计时和"什么时候该记一笔"，这边管"记完了怎么读出来"。

import { reactive } from 'vue'
import { tauriApi } from '../api/tauri'
import {
  countIn,
  fmtMinutes,
  minutesIn,
  normalizeFocusData,
  statsOfTodo,
  type FocusRecord,
  type TodoFocusStats,
} from '../core/focusLog'

export const focusLogStore = reactive({
  records: [] as FocusRecord[],
  loading: false,
  loaded: false,
  error: '',
})

/**
 * 读一次专注记录。**幂等**：已经读过了就不再读，除非 `force`。
 *
 * 为什么默认不重复读：这个文件只有本应用会写，界面上自己写的那些
 * 已经通过 `addRecord` 进了内存，没必要每次切页都去碰盘。
 * 唯一的例外是**从备份恢复之后** —— 那时盘上换了一份，必须强制重读。
 */
export async function loadFocusLog(force = false): Promise<void> {
  // 正在读就别再发一次（两个页面同时挂载时会撞上）—— 读的是同一份盘上内容
  if (focusLogStore.loading) return
  if (focusLogStore.loaded && !force) return
  focusLogStore.loading = true
  focusLogStore.error = ''
  const data = await tauriApi.focusLogRead()
  // 读回来的东西一律过一遍容错：坏记录丢掉，但**绝不因为它把整份读没**
  focusLogStore.records = normalizeFocusData(data).records
  focusLogStore.loaded = true
  focusLogStore.loading = false
}

/**
 * 记一条并立刻进内存（不用再读一次盘）。
 * 返回 `null` 表示成功，否则是一句给用户看的话 —— 失败必须说出来，
 * 否则用户只会看到"番茄钟数了、统计里却没有"。
 */
export async function addRecord(record: FocusRecord): Promise<string | null> {
  const err = await tauriApi.focusLogAppend(record)
  if (err === null) focusLogStore.records.push(record)
  else focusLogStore.error = err
  return err
}

/** 区间内的专注分钟数（区间形状与回顾页的 `ReviewRange` 兼容：只要有 from / to） */
export function focusMinutesIn(range: { from: string; to: string }): number {
  return minutesIn(focusLogStore.records, range)
}

export function focusCountIn(range: { from: string; to: string }): number {
  return countIn(focusLogStore.records, range)
}

/** 某条待办累计的番茄数与分钟数 */
export function focusStatsOfTodo(todoId: string): TodoFocusStats {
  return statsOfTodo(focusLogStore.records, todoId)
}

/** 把分钟数说成人话（回顾页用） */
export { fmtMinutes }
