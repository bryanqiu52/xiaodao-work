// 专注记录：本应用自己的数据，存 `数据根\专注记录.json`。
//
// **为什么不写进 `待办.json`**：那份清单是三方共写的（本应用 + DSH 插件 + AI），
// 加字段等于改契约，另外两方都得跟着动。而"这条待办花了几个番茄"是本应用自己的账，
// 放自己家文件里更合适。代价是小刀在 AI 侧看不到专注时长 —— 这个取舍是明确选定的。
//
// 日期一律是本地的 `YYYY-MM-DD`，比较走字符串（同 review.ts 的理由：
// `new Date('2026-09-25')` 按 UTC 解析，跨时区会偏一天）。

export interface FocusRecord {
  id: string
  /** 完成时刻，本地 ISO */
  at: string
  /** 本地日期 `YYYY-MM-DD` —— 统计全靠它，所以单独存一格，不从 `at` 现算 */
  date: string
  /** 这一段的分钟数（取当时配置的专注时长） */
  minutes: number
  /** 挂在哪条待办上；没挂就是空串 */
  todoId: string
  /**
   * **冗余存一份待办标题**。待办被改名或软删除之后，历史统计还能显示当时干的是什么；
   * 只存 id 的话回顾页会出现一片无名记录。
   */
  todoTitle: string
}

export interface FocusData {
  version: number
  records: FocusRecord[]
}

/** 与 Rust 侧 `focus_log.rs` 的 `FocusData.version` 对齐 */
export const FOCUS_VERSION = 1

export function emptyFocusData(): FocusData {
  return { version: FOCUS_VERSION, records: [] }
}

function asString(v: unknown, fallback = ''): string {
  return typeof v === 'string' ? v : fallback
}

/** 读盘容错：认不出来的记录直接丢掉，**绝不因为它把整份记录读没** */
export function normalizeFocusData(raw: unknown): FocusData {
  if (typeof raw !== 'object' || raw === null) return emptyFocusData()
  const list = (raw as { records?: unknown }).records
  if (!Array.isArray(list)) return emptyFocusData()
  const records: FocusRecord[] = []
  for (const entry of list) {
    if (typeof entry !== 'object' || entry === null) continue
    const r = entry as Record<string, unknown>
    const minutesRaw = typeof r.minutes === 'number' ? r.minutes : Number(r.minutes)
    records.push({
      id: asString(r.id),
      at: asString(r.at),
      date: asString(r.date),
      minutes: Number.isFinite(minutesRaw) && minutesRaw > 0 ? Math.round(minutesRaw) : 0,
      todoId: asString(r.todoId),
      todoTitle: asString(r.todoTitle),
    })
  }
  return { version: FOCUS_VERSION, records }
}

interface DayRange {
  from: string
  to: string
}

function inRange(date: string, range: DayRange): boolean {
  return date.length > 0 && date >= range.from && date <= range.to
}

/** 区间内的番茄个数 */
export function countIn(records: readonly FocusRecord[], range: DayRange): number {
  return records.filter((r) => inRange(r.date, range)).length
}

/** 区间内的专注总分钟数 */
export function minutesIn(records: readonly FocusRecord[], range: DayRange): number {
  return records.reduce((sum, r) => (inRange(r.date, range) ? sum + r.minutes : sum), 0)
}

export interface TodoFocusStats {
  count: number
  minutes: number
}

/** 某条待办累计了几个番茄、多少分钟 */
export function statsOfTodo(records: readonly FocusRecord[], todoId: string): TodoFocusStats {
  if (todoId.length === 0) return { count: 0, minutes: 0 }
  let count = 0
  let minutes = 0
  for (const r of records) {
    if (r.todoId !== todoId) continue
    count += 1
    minutes += r.minutes
  }
  return { count, minutes }
}

/** 分钟数说成人话：不满 60 就报分钟，满了报"X 小时 Y 分" */
export function fmtMinutes(minutes: number): string {
  if (minutes <= 0) return '0 分钟'
  if (minutes < 60) return `${minutes} 分钟`
  const h = Math.floor(minutes / 60)
  const m = minutes % 60
  return m === 0 ? `${h} 小时` : `${h} 小时 ${m} 分`
}
