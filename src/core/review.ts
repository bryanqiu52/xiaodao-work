// 回顾：把清单里已经攒下的数据算成人看得懂的数字。纯函数，不碰 DOM、不碰 invoke。
//
// 两条口径是硬的，改之前先想清楚：
//
// 1. **一律先按 `deletedAt === null` 过滤**。软删除的条目是用户主动移出的，
//    统计里混进它们，数字会跟列表上看到的对不上（`liveItems` 是所有对外输出的共同底数）。
// 2. **日期比较走 `YYYY-MM-DD` 字符串**，不用 `Date` 比大小 —— 字符串比较天然按本地日历走，
//    而 `new Date('2026-09-25')` 是按 UTC 解析的，跨时区会偏一天（这个坑仓库里踩过）。

import { DEFAULT_STATUS, OWNER_AGENT, STATUS_RANK } from './constants'
import type { Status, TodoItem } from './types'
import { isOverdue, todayLocal, weekEndLocal } from './view'

export type ReviewRangeKind = 'week' | 'month'

export interface ReviewRange {
  kind: ReviewRangeKind
  /** 「本周」/「本月」 */
  label: string
  /** 「9月22日 – 9月28日」，给副标题用 */
  span: string
  /** 含头含尾，都是 `YYYY-MM-DD` */
  from: string
  to: string
}

const WEEKDAYS = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']

/** `YYYY-MM-DD` → `M月D日` */
export function dayLabel(day: string): string {
  const [, m, d] = day.split('-').map(Number)
  return `${m ?? ''}月${d ?? ''}日`
}

/** `YYYY-MM-DD` → `M月D日 周三` */
export function dayWeekLabel(day: string): string {
  const [y, m, d] = day.split('-').map(Number)
  const dt = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1)
  return `${m ?? ''}月${d ?? ''}日 ${WEEKDAYS[dt.getDay()] ?? ''}`
}

/** 本月最后一天。`new Date(y, m, 0)` 取的是"下月第 0 天"，正好是本月最后一天 */
export function monthEnd(ym: string): string {
  const [y, m] = ym.split('-').map(Number)
  const dt = new Date(y ?? 1970, m ?? 1, 0)
  return `${ym}-${String(dt.getDate()).padStart(2, '0')}`
}

/**
 * 本周（**周一起算**，跟国内日历一致）or 本月的时间区间。
 * `today` 可传，便于测试与"以某天为基准"的复用。
 */
export function rangeOf(kind: ReviewRangeKind, today: string = todayLocal()): ReviewRange {
  if (kind === 'month') {
    const ym = today.slice(0, 7)
    const from = `${ym}-01`
    const to = monthEnd(ym)
    return { kind, label: '本月', span: `${dayLabel(from)} – ${dayLabel(to)}`, from, to }
  }
  const to = weekEndLocal(today)
  const [y, m, d] = today.split('-').map(Number)
  const dt = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1)
  // 距本周一的天数 = (getDay() + 6) % 7（getDay: 0=周日 … 6=周六）
  dt.setDate(dt.getDate() - ((dt.getDay() + 6) % 7))
  const from = `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}-${String(dt.getDate()).padStart(2, '0')}`
  return { kind, label: '本周', span: `${dayLabel(from)} – ${dayLabel(to)}`, from, to }
}

/** 这一天的 `YYYY-MM-DD` 落在区间里没 */
export function inRange(day: string, range: ReviewRange): boolean {
  return day.length > 0 && day >= range.from && day <= range.to
}

/** 时间戳取到日（`2026-09-25T10:32:00.000Z` → `2026-09-25`） */
function stampDay(stamp: string | null): string {
  return stamp === null ? '' : stamp.slice(0, 10)
}

/** 未删除的条目 —— 所有统计的共同底数 */
function liveOf(items: readonly TodoItem[]): TodoItem[] {
  return items.filter((i) => i.deletedAt === null)
}

export interface ReviewOverview {
  /** 区间内完成的条数 */
  doneCount: number
  /** 区间内新建的条数 */
  createdCount: number
  /** 当前还欠着的（未完成）条数 */
  pendingCount: number
  /** 还欠着的里面已过期的条数 */
  overdueCount: number
  /** 区间内完成那些的平均停留天数；没有样本返回 null（不是 0） */
  avgStayDays: number | null
}

export function overviewOf(items: readonly TodoItem[], range: ReviewRange): ReviewOverview {
  const live = liveOf(items)
  let doneCount = 0
  let createdCount = 0
  let pendingCount = 0
  let overdueCount = 0
  let staySum = 0
  let staySamples = 0

  for (const item of live) {
    if (inRange(stampDay(item.createdAt), range)) createdCount += 1
    if (item.doneAt === null) {
      pendingCount += 1
      if (isOverdue(item)) overdueCount += 1
      continue
    }
    if (!inRange(stampDay(item.doneAt), range)) continue
    doneCount += 1
    const start = Date.parse(item.createdAt)
    const end = Date.parse(item.doneAt)
    if (Number.isNaN(start) || Number.isNaN(end) || end < start) continue
    staySum += (end - start) / 86_400_000
    staySamples += 1
  }

  // 保留一位小数：2.3 天有信息量，2.3458712 天没有
  const avgStayDays = staySamples === 0 ? null : Math.round((staySum / staySamples) * 10) / 10
  return { doneCount, createdCount, pendingCount, overdueCount, avgStayDays }
}

export interface DomainStat {
  domain: string
  /** 区间内完成的 */
  done: number
  /** 现在还欠着的 */
  pending: number
  /** done + pending */
  total: number
  /** 完成率，0–100 的整数；total 为 0 时为 0 */
  rate: number
}

/**
 * 按领域分：区间内完成了几条、还欠几条、完成率多少。
 *
 * **区间外完成的那些不计入** —— 它们既不是"这段时间干的活"，也不是"还欠着的"，
 * 塞进任何一边都会让数字失真。
 */
export function domainStats(items: readonly TodoItem[], range: ReviewRange): DomainStat[] {
  const map = new Map<string, { done: number; pending: number }>()
  for (const item of liveOf(items)) {
    const slot = map.get(item.domain) ?? { done: 0, pending: 0 }
    if (item.doneAt === null) slot.pending += 1
    else if (inRange(stampDay(item.doneAt), range)) slot.done += 1
    else continue
    map.set(item.domain, slot)
  }
  return [...map.entries()]
    .map(([domain, s]) => {
      const total = s.done + s.pending
      return { domain, done: s.done, pending: s.pending, total, rate: total === 0 ? 0 : Math.round((s.done / total) * 100) }
    })
    .filter((s) => s.total > 0)
    .sort((a, b) => (b.done === a.done ? b.total - a.total : b.done - a.done))
}

export interface DomainPending {
  domain: string
  count: number
  /** 其中已过期的条数 */
  overdue: number
}

/** 还欠着的按领域分布（过期条数单独标出来 —— 那是最该先动的一批） */
export function pendingByDomain(items: readonly TodoItem[]): DomainPending[] {
  const map = new Map<string, DomainPending>()
  for (const item of liveOf(items)) {
    if (item.doneAt !== null) continue
    const slot = map.get(item.domain) ?? { domain: item.domain, count: 0, overdue: 0 }
    slot.count += 1
    if (isOverdue(item)) slot.overdue += 1
    map.set(item.domain, slot)
  }
  return [...map.values()].sort((a, b) => b.count - a.count)
}

export interface StatusCount {
  status: Status
  count: number
}

/**
 * 小刀那一摊：还在手上的按状态分布。
 * 排序用 `STATUS_RANK`（进行中 → 等你回话 → 排队 → 完成 → 暂停垫底），
 * 跟小刀视图列表的顺序一致 —— 两处口径不一样的话，用户会以为哪里算错了。
 */
export function agentBreakdown(items: readonly TodoItem[]): StatusCount[] {
  const map = new Map<Status, number>()
  for (const item of liveOf(items)) {
    if (item.owner !== OWNER_AGENT || item.doneAt !== null) continue
    map.set(item.status, (map.get(item.status) ?? 0) + 1)
  }
  return [...map.entries()]
    .map(([status, count]) => ({ status, count }))
    .sort((a, b) => {
      const ra = STATUS_RANK[a.status] ?? STATUS_RANK[DEFAULT_STATUS] ?? 2
      const rb = STATUS_RANK[b.status] ?? STATUS_RANK[DEFAULT_STATUS] ?? 2
      return ra === rb ? b.count - a.count : ra - rb
    })
}
