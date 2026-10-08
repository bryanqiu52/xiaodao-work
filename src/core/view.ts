// 视图层纯逻辑：排序 / 搜索 / 筛选 / 计数。
//
// 全部是纯函数、不碰 DOM，所以能单独测，也能被搜索和筛选复用同一份口径。

import {
  ID_SHORT_MAX,
  OWNER_AGENT,
  PRIORITY_RANK,
  STATUS_RANK,
  DEFAULT_STATUS,
} from './constants'
import type { FilterValue, TodoItem } from './types'

/** 期限的日粒度（排序用）：没期限返回空串 */
export function dueDay(item: TodoItem): string {
  const d = item.dueAt
  if (typeof d !== 'string' || d.length === 0) return ''
  return d.slice(0, 10)
}

export function isAgentItem(item: TodoItem): boolean {
  return item.owner === OWNER_AGENT
}

/** 本地今天（`YYYY-MM-DD`）。用本地的"今天"，不是 UTC 的 —— 用户的今天就是他钟表上的今天 */
export function todayLocal(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** 是否过期（有期限且已过当天） */
export function isOverdue(item: TodoItem): boolean {
  const day = dueDay(item)
  return day !== '' && day < todayLocal()
}

/**
 * 是不是"今天到期"。
 * 和 `isOverdue` 配对使用：这两类正是会触发提醒的，也是「稍后再说」该出现的场合。
 */
export function isDueToday(item: TodoItem): boolean {
  const day = dueDay(item)
  return day !== '' && day === todayLocal()
}

/**
 * 本周最后一天（周日）的 `YYYY-MM-DD`。
 * **周一算一周之首** —— 跟国内日历一致，周日是这周的最后一天而不是第一天。
 *
 * 正主是「回顾」页（`core/review.ts` 用它切本周区间）。
 * ⚠️ **它曾被摆在"按期限分组"那段里，但那个功能 2026-09-27 已经删了 ——
 * 这个函数别跟着一起清掉**，删了回顾页的"本周"会当场报错（这次差点踩到）。
 */
export function weekEndLocal(today: string = todayLocal()): string {
  const [y, m, d] = today.split('-').map(Number)
  const dt = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1)
  // getDay(): 0=周日 … 6=周六。距上一个周一的天数 = (getDay() + 6) % 7，
  // 再补到那一天就是本周日
  dt.setDate(dt.getDate() + (6 - ((dt.getDay() + 6) % 7)))
  return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}-${String(dt.getDate()).padStart(2, '0')}`
}

/**
 * 组内次级顺序：优先级 高→中→低、期限近→远（无期限最后）、创建时间新→旧。
 */
export function compareWithinGroup(a: TodoItem, b: TodoItem): number {
  const rankA = PRIORITY_RANK[a.priority] ?? 9
  const rankB = PRIORITY_RANK[b.priority] ?? 9
  if (rankA !== rankB) return rankA - rankB

  const dueA = dueDay(a)
  const dueB = dueDay(b)
  if (dueA !== dueB) {
    if (dueA === '') return 1
    if (dueB === '') return -1
    return dueA < dueB ? -1 : 1
  }

  if (a.createdAt === b.createdAt) return 0
  return a.createdAt < b.createdAt ? 1 : -1
}

/**
 * 我的视图：未完成按 优先级 → 期限近远 → 创建时间新→旧 排。
 *
 * 小刀视图：先按状态分组 progress → waiting → todo → done → paused
 * （进行中最前、暂停最后），**同一组内**再用上面那套。
 */
export function makeCompareItems(agentView: boolean): (a: TodoItem, b: TodoItem) => number {
  if (!agentView) return compareWithinGroup
  return (a, b) => {
    const rankA = STATUS_RANK[a.status] ?? STATUS_RANK[DEFAULT_STATUS] ?? 2
    const rankB = STATUS_RANK[b.status] ?? STATUS_RANK[DEFAULT_STATUS] ?? 2
    if (rankA !== rankB) return rankA - rankB
    return compareWithinGroup(a, b)
  }
}

/** 列表数据：先按归属切一刀，再筛掉已完成、按当前视图的规则排序 */
export function visibleItems(items: readonly TodoItem[], owner: string): TodoItem[] {
  const compare = makeCompareItems(owner === OWNER_AGENT)
  return items
    .filter((item) => isAgentItem(item) === (owner === OWNER_AGENT))
    .filter((item) => item.doneAt === null)
    .sort(compare)
}

/** 已完成的那些：同一归属，最近完成的排在上面；doneAt 缺失的垫底 */
export function completedItems(items: readonly TodoItem[], owner: string): TodoItem[] {
  return items
    .filter((item) => isAgentItem(item) === (owner === OWNER_AGENT))
    .filter((item) => item.doneAt !== null)
    .sort((a, b) => {
      const doneA = a.doneAt ?? ''
      const doneB = b.doneAt ?? ''
      if (doneA === doneB) return 0
      if (doneA === '') return 1
      if (doneB === '') return -1
      return doneA < doneB ? 1 : -1
    })
}

// ── 编号 ──────────────────────────────────────────────────────────────────

/** 编号显示形态：短的全留，长的截中间 */
export function shortId(id: string): string {
  return id.length > ID_SHORT_MAX ? `${id.slice(0, 7)}…${id.slice(-5)}` : id
}

/**
 * 截断形态的编号能不能对上这条 id（供搜索用）。
 *
 * 照着屏幕敲的时候，可能连中间的省略号一起敲进来，也可能只记得尾巴。
 * 这里把**首段和尾段分别**拿去比对：
 * - 关键词含 `…`（或 `...`）→ 拆成两半，**两半都得对上**才算 —— 否则 `xx…yy`
 *   会匹配所有含 `xx` 的条目，比不搜还乱；
 * - 不含省略号 → 首段或尾段任一命中即可（`qn9hs` 这种"只记得尾巴"很常见）。
 *
 * 只在**会被截断的长 id** 上生效；短编号走普通子串匹配。
 */
export function matchesShortId(id: string, needle: string): boolean {
  if (id.length <= ID_SHORT_MAX) return false
  const head = id.slice(0, 7).toLowerCase()
  const tail = id.slice(-5).toLowerCase()
  // 别给这个正则加捕获组 —— `split` 遇到捕获组会把组本身也塞进结果里，
  // 于是 `a…b` 会拆成 ['a','…','b']，解构拿到的"右半"就成了省略号本身。
  const dots = /…|\.\.\./
  if (dots.test(needle)) {
    const [left = '', right = ''] = needle.split(dots)
    return head.includes(left) && tail.includes(right)
  }
  return head.includes(needle) || tail.includes(needle)
}

/**
 * 搜索关键词命中**编号 / 标题 / 正文**（本地、不区分大小写）。
 *
 * 编号放进搜索是硬需求：用法是**从别处拿到编号，回来在面板上找那条**。
 * - 编号整体匹配：`imp-037` 能搜到，`037` 也能（子串）；
 * - 长 id 的截断形态也能搜（`matchesShortId`）；
 * - 空关键词一律全收。
 */
export function matchesKeyword(item: TodoItem, keyword: string): boolean {
  if (keyword === '') return true
  const needle = keyword.toLowerCase()
  if (item.id.toLowerCase().includes(needle)) return true
  if (matchesShortId(item.id, needle)) return true
  return item.title.toLowerCase().includes(needle) || item.detail.toLowerCase().includes(needle)
}

export function matchesDomain(item: TodoItem, domain: FilterValue): boolean {
  return domain === null || item.domain === domain
}

export function matchesPriority(item: TodoItem, priority: FilterValue): boolean {
  return priority === null || item.priority === priority
}

export function matchesStatus(item: TodoItem, status: FilterValue): boolean {
  return status === null || item.status === status
}

// ── 计数 ──────────────────────────────────────────────────────────────────

/**
 * 某个归属下**未完成**的条数（顶部两个 tab 上的数字）。
 * 只按 owner + doneAt 算：搜索 / 领域 / 权重一概不参与，切筛选时数字不跳。
 */
export function countPendingOwned(items: readonly TodoItem[], owner: string): number {
  return items.filter((item) => isAgentItem(item) === (owner === OWNER_AGENT) && item.doneAt === null)
    .length
}

/**
 * 按某个字段数一数（喂给筛选标签上的数字）。
 *
 * **计数口径**：传入的 `items` 已经是「当前归属 + 其它已选条件」的结果，
 * 但不包含本字段自身的筛选 —— 所以选中一档后别的数字不缩水
 * （比如选了"高"，"中""低"的数字还是原来那么多，方便直接改选）。
 */
export function countBy(items: readonly TodoItem[], field: 'priority' | 'status' | 'domain'): Record<string, number> {
  const counts: Record<string, number> = {}
  for (const item of items) {
    const key = item[field]
    counts[key] = (counts[key] ?? 0) + 1
  }
  return counts
}
