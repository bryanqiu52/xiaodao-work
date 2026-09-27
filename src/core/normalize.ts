// 归一化：把盘上 / AI 手写的任意形状收敛成 TodoItem。
//
// 一条贯穿全部函数的原则：**留痕宁可少一条，也不能留下看不懂的**。
// 所以残缺的流水 / 审批条目是**整条丢弃**，而不是补个默认值凑数 ——
// 一条没头没尾的留痕比没有留痕更糟，它会让人以为发生过什么。

import {
  activeDomainOptions,
  fallbackDomain,
  DOMAIN_OPTIONS,
  LEGACY_DOMAINS,
  OWNER_OPTIONS,
  PRIORITY_OPTIONS,
  STATUS_OPTIONS,
} from './constants'
import type {
  Domain,
  Owner,
  Priority,
  Status,
  TodoItem,
  TodoOrigin,
  TodoReview,
  TodoTrail,
} from './types'

export function normalizeDomain(value: unknown): Domain {
  const text = typeof value === 'string' ? value.trim().toLowerCase() : ''
  // 认**当前生效**的分类（用户在设置里自定义过的那套），而不是写死的内置 6 类 ——
  // 否则自定义分类一进来就被打回 personal，设置里加了等于白加
  if ((activeDomainOptions() as readonly string[]).includes(text)) return text as Domain
  // 内置那 6 类永远认（配置还没载入时也得能干活）
  if ((DOMAIN_OPTIONS as readonly string[]).includes(text)) return text as Domain
  // 兜底到当前列表的第一项，而不是写死的 personal：
  // 分类现在是用户可配的（还能删），personal 未必在他的列表里
  return LEGACY_DOMAINS[text] ?? fallbackDomain()
}

export function normalizeOwner(value: unknown): Owner {
  const text = typeof value === 'string' ? value.trim().toLowerCase() : ''
  return (OWNER_OPTIONS as readonly string[]).includes(text) ? (text as Owner) : 'user'
}

export function normalizeStatus(value: unknown): Status {
  const text = typeof value === 'string' ? value.trim().toLowerCase() : ''
  return (STATUS_OPTIONS as readonly string[]).includes(text) ? (text as Status) : 'todo'
}

export function normalizePriority(value: unknown): Priority {
  const text = typeof value === 'string' ? value.trim().toLowerCase() : ''
  return (PRIORITY_OPTIONS as readonly string[]).includes(text) ? (text as Priority) : 'mid'
}

/** 审批记录：旧数据没有这个字段 → 补空数组；残缺或 action 非法的整条丢弃 */
export function normalizeReviews(value: unknown): TodoReview[] {
  if (!Array.isArray(value)) return []
  const out: TodoReview[] = []
  for (const raw of value) {
    if (typeof raw !== 'object' || raw === null) continue
    const entry = raw as { action?: unknown; at?: unknown; comment?: unknown }
    const action =
      entry.action === 'approved' || entry.action === 'rejected' || entry.action === 'shelved'
        ? entry.action
        : null
    if (action === null) continue
    out.push({
      action,
      at: typeof entry.at === 'string' && entry.at.length > 0 ? entry.at : new Date().toISOString(),
      comment: typeof entry.comment === 'string' ? entry.comment : '',
    })
  }
  return out
}

/** 流水：与审批同一原则 —— 残缺条目整条丢弃 */
export function normalizeTrail(value: unknown): TodoTrail[] {
  if (!Array.isArray(value)) return []
  const kinds: readonly TodoTrail['kind'][] = [
    'create',
    'status',
    'edit',
    'review',
    'transfer',
    'comment',
  ]
  const out: TodoTrail[] = []
  for (const raw of value) {
    if (typeof raw !== 'object' || raw === null) continue
    const entry = raw as { kind?: unknown; at?: unknown; text?: unknown; by?: unknown }
    const kind = kinds.find((k) => k === entry.kind)
    if (kind === undefined) continue
    const text = typeof entry.text === 'string' ? entry.text.trim() : ''
    if (text.length === 0) continue
    out.push({
      kind,
      at: typeof entry.at === 'string' && entry.at.length > 0 ? entry.at : new Date().toISOString(),
      text,
      by: typeof entry.by === 'string' ? entry.by : '',
    })
  }
  return out
}

/** 脉络：旧数据没有 → 补空结构（不是 null，省得下游到处判空） */
export function normalizeOrigin(value: unknown): TodoOrigin {
  if (typeof value !== 'object' || value === null) return { from: '', why: '' }
  const entry = value as { from?: unknown; why?: unknown }
  return {
    from: typeof entry.from === 'string' ? entry.from.trim() : '',
    why: typeof entry.why === 'string' ? entry.why.trim() : '',
  }
}

/** 字符串数组：去空、去重、保序。用于 `relates` 与 `files` */
export function normalizeStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  const seen = new Set<string>()
  const out: string[] = []
  for (const raw of value) {
    if (typeof raw !== 'string') continue
    const text = raw.trim()
    if (text.length === 0 || seen.has(text)) continue
    seen.add(text)
    out.push(text)
  }
  return out
}

/**
 * 来源会话 id 规范成带 `session-` 前缀的形式。
 * 桌面端没有会话可跳，但这个字段还在（AI 仍会写），保留规范化只为数据兼容。
 */
export function normalizeSource(source: string): string {
  return source === '' || source.startsWith('session-') ? source : `session-${source}`
}

/** 类型守卫：只校验主键是字符串、标题非空 —— 其余字段一律交给归一化兜底 */
export function validItem(x: unknown): x is TodoItem {
  return (
    typeof x === 'object' &&
    x !== null &&
    typeof (x as TodoItem).id === 'string' &&
    typeof (x as TodoItem).title === 'string' &&
    (x as TodoItem).title.length > 0
  )
}

/**
 * 把一条任意形状的原始记录整理成 TodoItem。
 *
 * 对应插件 `loadFromDisk()` 里那个 map：**每条记录的每个字段都要过一遍归一化**，
 * 缺字段一律补默认值，绝不让 undefined 流到 UI 里。
 */
export function normalizeItem(raw: unknown): TodoItem | null {
  if (!validItem(raw)) return null
  const item = raw as Partial<TodoItem> & { id: string; title: string }
  return {
    id: item.id,
    title: item.title,
    detail: typeof item.detail === 'string' ? item.detail : '',
    summary: typeof item.summary === 'string' ? item.summary : '',
    domain: normalizeDomain(item.domain),
    owner: normalizeOwner(item.owner),
    status: normalizeStatus(item.status),
    priority: normalizePriority(item.priority),
    dueAt: typeof item.dueAt === 'string' && item.dueAt.length > 0 ? item.dueAt : null,
    createdAt: typeof item.createdAt === 'string' ? item.createdAt : new Date().toISOString(),
    doneAt: typeof item.doneAt === 'string' && item.doneAt.length > 0 ? item.doneAt : null,
    deletedAt: typeof item.deletedAt === 'string' && item.deletedAt.length > 0 ? item.deletedAt : null,
    reviews: normalizeReviews(item.reviews),
    trail: normalizeTrail(item.trail),
    origin: normalizeOrigin(item.origin),
    relates: normalizeStringList(item.relates),
    files: normalizeStringList(item.files),
    source: normalizeSource(typeof item.source === 'string' ? item.source : ''),
    link: typeof item.link === 'string' ? item.link : '',
  }
}
