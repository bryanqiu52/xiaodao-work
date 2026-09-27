// 改一条已有记录：**这是"手动改状态"的唯一入口**（面板行内的状态 chip 走的也是它）。
//
// 所以流水必须记在这里 —— 曾有过"以为点了暂停、实际变成重做"的事故，
// 当时没有任何留痕，事后只能靠产出文件的时间戳反推。现在每次变更都留一句人话。

import { normalizeDomain, normalizeOrigin, normalizeOwner, normalizePriority, normalizeStatus, normalizeStringList } from './normalize'
import { recordTrail, STATUS_TEXT } from './rules'
import type { TodoItem, TodoOrigin } from './types'

export interface PatchArgs {
  title?: unknown
  summary?: unknown
  detail?: unknown
  domain?: unknown
  owner?: unknown
  status?: unknown
  priority?: unknown
  due?: unknown
  link?: unknown
  origin?: unknown
  relates?: unknown
  files?: unknown
  trailNote?: unknown
}

/**
 * 把外部传来的脉络参数解析成 `applyPatch` 认的形状。
 *
 * 约定：**传 `none` = 清空该项；未传（undefined）= 不动**。
 * 不用空串当清空信号 —— 那是"手滑发了个空串"和"我要清空"分不开的写法。
 *
 * `origin` 的 from / why 是**两格并排的独立格子**：只传一格时另一格不动。
 * （早先的写法会把没传的那格写成空串，等于顺手抹掉了交代的来龙去脉。）
 */
export function parseContextPatch(args: {
  originFrom?: string
  originWhy?: string
  relates?: string
  files?: string
}): {
  origin?: { from?: string | null; why?: string | null }
  relates?: string[]
  files?: string[]
} {
  const out: {
    origin?: { from?: string | null; why?: string | null }
    relates?: string[]
    files?: string[]
  } = {}
  const splitList = (value: string): string[] =>
    value
      .split(/[,，\n]/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0)
  const isClear = (value: string): boolean => value.trim().toLowerCase() === 'none'
  const cell = (value: string): string | null => (isClear(value) ? null : value.trim())

  if (args.originFrom !== undefined) out.origin = { from: cell(args.originFrom) }
  if (args.originWhy !== undefined) {
    out.origin = { ...(out.origin ?? {}), why: cell(args.originWhy) }
  }
  if (args.relates !== undefined) {
    out.relates = isClear(args.relates) ? [] : splitList(args.relates)
  }
  if (args.files !== undefined) {
    out.files = isClear(args.files) ? [] : splitList(args.files)
  }
  return out
}

/** `undefined` = 不动；`null` = 明确清空 */
type OriginPatch = { from?: string | null; why?: string | null }

/**
 * 把一批可选字段写进一条已有记录（undefined 表示不动）。
 *
 * 变更要么记一条 comment 流水（带留言时），要么记一条 edit 流水（改了字段时）。
 * **不会静默改数据** —— 改了什么都不留痕，等于这条路径不可追溯。
 */
export function applyPatch(item: TodoItem, args: PatchArgs): void {
  const changes: string[] = []

  if (args.title !== undefined) {
    const title = String(args.title).trim()
    if (title.length > 0 && title !== item.title) {
      item.title = title
      changes.push('标题')
    }
  }
  if (args.summary !== undefined) {
    const summary = String(args.summary).trim()
    if (summary !== item.summary) {
      item.summary = summary
      changes.push('概要')
    }
  }
  if (args.detail !== undefined) {
    const detail = String(args.detail)
    if (detail !== item.detail) {
      item.detail = detail
      changes.push('详情')
    }
  }
  if (args.domain !== undefined) {
    const domain = normalizeDomain(args.domain)
    if (domain !== item.domain) {
      item.domain = domain
      changes.push('领域')
    }
  }
  if (args.owner !== undefined) {
    const owner = normalizeOwner(args.owner)
    if (owner !== item.owner) {
      item.owner = owner
      changes.push('归属')
    }
  }
  if (args.status !== undefined) {
    const next = normalizeStatus(args.status)
    if (next !== item.status) {
      // 完成态必须自洽：切到 done 就补 doneAt，切离 done 就清掉 doneAt（相当于"恢复"）
      if (next === 'done' && item.doneAt === null) item.doneAt = new Date().toISOString()
      if (next !== 'done' && item.doneAt !== null) item.doneAt = null
      const from = STATUS_TEXT[item.status] ?? item.status
      const to = STATUS_TEXT[next] ?? next
      recordTrail(item, 'status', `状态「${from}」→「${to}」`, '')
      item.status = next
    }
  }
  if (args.priority !== undefined) {
    const priority = normalizePriority(args.priority)
    if (priority !== item.priority) {
      item.priority = priority
      changes.push('优先级')
    }
  }
  if (args.due !== undefined) {
    const due = String(args.due).trim()
    const next = due.length > 0 ? due : null
    if (next !== item.dueAt) {
      item.dueAt = next
      changes.push('期限')
    }
  }
  if (args.link !== undefined) {
    const link = String(args.link)
    if (link !== item.link) {
      item.link = link
      changes.push('链接')
    }
  }

  // 脉络三件套：AI 写的部分，改了也留一笔 —— 否则"这条什么时候被挂上来源的"无从追溯。
  // origin 逐格合并：`undefined` = 不动，`null` = 明确清空，字符串 = 覆盖。
  if (args.origin !== undefined) {
    const patch = args.origin as OriginPatch
    const from = patch.from === null ? '' : patch.from ?? item.origin.from
    const why = patch.why === null ? '' : patch.why ?? item.origin.why
    const merged: TodoOrigin = normalizeOrigin({ from, why })
    if (merged.from !== item.origin.from || merged.why !== item.origin.why) {
      item.origin = merged
      changes.push('脉络')
    }
  }
  if (args.relates !== undefined) {
    const relates = normalizeStringList(args.relates)
    if (relates.join('\u0000') !== item.relates.join('\u0000')) {
      item.relates = relates
      changes.push('关联待办')
    }
  }
  if (args.files !== undefined) {
    const files = normalizeStringList(args.files)
    if (files.join('\u0000') !== item.files.join('\u0000')) {
      item.files = files
      changes.push('关联文件')
    }
  }

  const note = args.trailNote !== undefined ? String(args.trailNote).trim() : ''
  if (note.length > 0) {
    recordTrail(item, 'comment', note, '')
  } else if (changes.length > 0) {
    recordTrail(item, 'edit', `修改了${changes.join('、')}`, '')
  }
}
