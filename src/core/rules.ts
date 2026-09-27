// 业务规则：完成态自洽、软删除、流水、审批、转交、新建。
//
// 这些是插件跑了很久、bug 日志里全是血泪的那部分。**改任何一条之前先看注释里的"为什么"**
// —— 每条都对应一次真实事故，看着别扭的写法多半是修过 bug 的痕迹。
//
// 约定：这里所有函数**直接改传入的 item**（与插件一致），不返回副本。
// 调用方负责先 `normalizeItem`、后整体写回。

import { STATUS_LABELS } from './constants'
import { normalizeDomain, normalizeStringList } from './normalize'
import type { ReviewAction, TodoItem, TodoTrail } from './types'

/**
 * 完成态的硬不变式：`(doneAt !== null) <=> (status === 'done')`。
 *
 * 修复前「完成」只写 `doneAt`、不写 `status`，盘上因此躺着 `doneAt` 有值
 * 而 `status` 仍是 `todo` 的矛盾记录。加载时一次性补齐 —— 幂等，已自洽的纹丝不动。
 *
 * 补 `doneAt` 时退回 **createdAt**，不编造"现在完成"。
 */
export function reconcileDoneState(list: TodoItem[]): { fixed: string[] } {
  const fixed: string[] = []
  for (const item of list) {
    if (item.doneAt !== null && item.status !== 'done') {
      item.status = 'done'
      fixed.push(item.id)
    } else if (item.status === 'done' && item.doneAt === null) {
      item.doneAt = item.createdAt
      fixed.push(item.id)
    }
  }
  return { fixed }
}

/** 加载后自检：仍然不对劲的地方打一条 warn，不静默吞掉 */
export function auditItems(list: TodoItem[]): string[] {
  const nowIso = new Date().toISOString()
  const problems: string[] = []
  for (const item of list) {
    if (item.createdAt > nowIso) problems.push(`${item.id} 的 createdAt 在未来（${item.createdAt}）`)
    if (item.doneAt !== null && item.doneAt > nowIso) problems.push(`${item.id} 的 doneAt 在未来（${item.doneAt}）`)
    if (item.doneAt !== null && item.deletedAt !== null) problems.push(`${item.id} 同时标了已完成与已删除`)
  }
  return problems
}

// ── 列表切片：所有对外输出的共同底数 ────────────────────────────────────────

/** 未被软删除的记录 —— **所有对外输出的共同底数** */
export function liveItems(items: readonly TodoItem[]): TodoItem[] {
  return items.filter((item) => item.deletedAt === null)
}

export function pendingItems(items: readonly TodoItem[]): TodoItem[] {
  return liveItems(items).filter((item) => item.doneAt === null)
}

export function doneItems(items: readonly TodoItem[]): TodoItem[] {
  return liveItems(items).filter((item) => item.doneAt !== null)
}

export function itemsForScope(items: readonly TodoItem[], scope: 'pending' | 'done' | 'all'): TodoItem[] {
  if (scope === 'done') return doneItems(items)
  if (scope === 'all') return liveItems(items)
  return pendingItems(items)
}

// ── 流水 ──────────────────────────────────────────────────────────────────

/**
 * 追加一条动作流水。**这是"事实"那一半，程序自己记，不靠谁自觉。**
 *
 * 每个改状态的入口都必须调它 —— 漏一个，那条路径就变成查不到的黑洞。
 */
export function recordTrail(item: TodoItem, kind: TodoTrail['kind'], text: string, by = ''): void {
  item.trail = [...item.trail, { kind, at: new Date().toISOString(), text, by }]
}

/** 状态的中文名，写流水时用 —— 让流水是"人话" */
export const STATUS_TEXT: Record<string, string> = STATUS_LABELS

// ── 三个终态 ──────────────────────────────────────────────────────────────

/**
 * 标记完成：`doneAt` 与 `status` 必须**同写**。
 * 只写其中一个，文件里就会留下自相矛盾的记录。
 */
export function markDone(item: TodoItem): void {
  item.doneAt = new Date().toISOString()
  item.status = 'done'
  recordTrail(item, 'status', '标为「已完成」', '')
}

/**
 * 软删除：只打时间戳，不把记录从文件里抹掉。
 *
 * 物理删除会让「这条是做完了还是被删了」永远无法回溯（`imp-032` 事件就栽在这儿：
 * 面板上点「完成」和点「删除」长得一模一样）。留痕之后，打开 JSON 能分辨这两种终态。
 */
export function markDeleted(item: TodoItem): void {
  item.deletedAt = new Date().toISOString()
  recordTrail(item, 'edit', '移出列表（软删除，记录留档）', '')
}

/**
 * 恢复：把已完成打回未完成。`doneAt` 与 `status` 仍然**同写**。
 *
 * 状态一律回到 `todo`（排队）：不记录状态历史，还原不出"完成前是哪个状态"，
 * 选 `todo` 是唯一自洽、也不会误导读者的取值。
 */
export function restoreItem(item: TodoItem): void {
  item.doneAt = null
  item.status = 'todo'
  recordTrail(item, 'status', '打回「排队」', '')
}

// ── 审批 ──────────────────────────────────────────────────────────────────

/**
 * 挂起：状态改 `paused`，并把理由**追加进 detail**。
 *
 * 为什么理由不能只留在 `reviews` 里：小刀下次开工只读 `status` 与 `detail`，
 * `reviews` 它并不看。理由不写进 detail，小刀就只知道"这条挂了"却不知道"为什么挂"。
 */
export function shelveItem(item: TodoItem, comment: string): void {
  item.status = 'paused'
  const why = comment.trim()
  if (why.length > 0) {
    const now = new Date()
    const stamp = `${pad2(now.getMonth() + 1)}-${pad2(now.getDate())} ${pad2(now.getHours())}:${pad2(now.getMinutes())}`
    item.detail = `${(item.detail ?? '').trim()}\n\n【老板挂起 ${stamp}】${why}`.trim()
  }
  recordTrail(item, 'status', '挂起为「已暂停」', '')
}

/**
 * 记一次审批并推进状态：
 * - 通过 → 议题结束（同 markDone）
 * - 退回 → 球踢回小刀重做（同 restoreItem，状态回 `todo`）
 * - 先挂起 → 议题不表态、挂起来（状态 `paused`）
 *
 * 「退回」与「先挂起」看着都像"我不同意"，但去向完全相反：退回是**让小刀再干一遍**，
 * 挂起是**让它别再动了**。曾有人点「退回」本意是"先停下"，结果小刀照 `todo` 又跑了一轮、
 * 产出文件被重写 —— 所以第三个出口必须存在，且状态落 `paused` 而不是回 `todo`。
 *
 * 顺序上**先留痕、后改状态**：万一状态后来被人手动改了，
 * "这一条审过、当时说了什么"依然查得到。
 */
export function recordReview(item: TodoItem, action: ReviewAction, comment: string): void {
  item.reviews = [...item.reviews, { action, at: new Date().toISOString(), comment }]
  const label = action === 'approved' ? '通过' : action === 'shelved' ? '先挂起' : '退回'
  const note = comment.length > 0 ? `：${comment}` : ''
  recordTrail(item, 'review', `老板${label}${note}`, '')
  if (action === 'approved') markDone(item)
  else if (action === 'shelved') shelveItem(item, comment)
  else restoreItem(item)
}

// ── 转交 ──────────────────────────────────────────────────────────────────

/**
 * 转交：一条待办在「我的」和「小刀的」之间换手。
 *
 * 两个方向的意义不一样，待遇也不一样：
 *
 * - 转给小刀 → 得说清「要小刀做什么」。note 追加进 detail（小刀下次开工读得到），
 *   并且状态拉回 `todo` —— 球已到小刀脚下，若还停在 `waiting`（等你拍板），
 *   它会一直卡着没人动，等于转了跟没转一样。
 * - 转给我 → 自己接手，不需要交代。只改归属，状态不动。
 *
 * 两个方向都往 detail 追加一行带时间戳的记录：这行字是小刀下次开工时
 * 唯一能看出「这条被转过手」的线索。
 */
export function recordTransfer(item: TodoItem, to: 'user' | 'agent', note: string): void {
  const now = new Date()
  const stamp = `${pad2(now.getMonth() + 1)}-${pad2(now.getDate())} ${pad2(now.getHours())}:${pad2(now.getMinutes())}`
  const label = to === 'agent' ? '老板交办' : '老板接手'
  // 转给小刀：note 是交代，空着就不留这行（没交代却转过去，等于让它猜，宁可留白）。
  // 转给我：不要求填任何东西，但仍落一行时间戳 —— 留痕是给将来回溯用的，不是给别人看的。
  const body = to === 'agent' ? note : note.length > 0 ? note : '自己接手安排'
  if (body.length > 0) {
    item.detail = `${(item.detail ?? '').trim()}\n\n【${label} ${stamp}】${body}`.trim()
  }
  item.owner = to
  if (to === 'agent' && item.status !== 'done') item.status = 'todo'
  recordTrail(
    item,
    'transfer',
    to === 'agent'
      ? `老板转交给小刀${note.length > 0 ? `：${note}` : ''}`
      : `老板收回自己办${note.length > 0 ? `：${note}` : ''}`,
    '',
  )
}

export function pad2(value: number): string {
  return value < 10 ? `0${value}` : String(value)
}

// ── 新建 ──────────────────────────────────────────────────────────────────

/** 新 id：`i<base36 时间戳>-<6 位随机>` */
export function nextId(): string {
  return `i${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

/**
 * 新建一条待办（只填标题的"快记"走这条路径，其余字段取默认值）。
 * 出生就记一条 create 流水 —— 否则"这条什么时候建的"只能靠 createdAt 猜。
 */
export function createItem(input: {
  title: string
  domain?: string
  owner?: 'user' | 'agent'
  status?: string
  priority?: string
  detail?: string
  summary?: string
  dueAt?: string | null
  link?: string
  /**
   * 产出文件 / 目录。**新建时就要能挂** —— 早先这两个字段只有 `applyPatch` 认，
   * 于是"新建时填的产出"会被静默丢掉（编辑时才生效），这种半截子最容易被当成 bug 反复报。
   */
  files?: string[]
  /** 关联待办 id */
  relates?: string[]
}): TodoItem {
  const now = new Date().toISOString()
  const item: TodoItem = {
    id: nextId(),
    title: input.title.trim(),
    detail: (input.detail ?? '').trim(),
    summary: (input.summary ?? '').trim(),
    domain: normalizeDomain(input.domain),
    owner: input.owner === 'agent' ? 'agent' : 'user',
    status: input.status === 'progress' ? 'progress' : 'todo',
    priority: input.priority === 'high' || input.priority === 'low' ? input.priority : 'mid',
    dueAt: input.dueAt && input.dueAt.length > 0 ? input.dueAt : null,
    createdAt: now,
    doneAt: null,
    deletedAt: null,
    reviews: [],
    trail: [],
    origin: { from: '', why: '' },
    relates: normalizeStringList(input.relates),
    files: normalizeStringList(input.files),
    source: '',
    link: (input.link ?? '').trim(),
  }
  recordTrail(item, 'create', '建了这条', '')
  return item
}
