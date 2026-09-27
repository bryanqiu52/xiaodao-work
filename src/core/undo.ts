// 撤销栈：纯逻辑，不依赖 Vue、不碰 invoke。
//
// **为什么撤销要收口在 store 的 `commit()` 上**：那是所有待办写操作的唯一闸门，
// 而"外部改动就该清栈"这个硬要求同样收口在 `writeItems` / `loadTodos` 两处。
// 挂在别处就得给九个写操作各配一遍，迟早漏一个。
//
// **为什么外部改动要清栈，而不是留着**：`待办.json` 是三方共写的（本应用 + DSH 插件 + AI）。
// 撤销的语义是"回到上一版"，而盘上被 AI 改过之后，栈里那份快照就不再是"上一版"，
// 而是**另一条线上过期的状态** —— 照它写回等于整份覆盖 AI 的改动，
// 那正是 `todo_io` 的指纹比对要拦的事。冲突时栈已失效，清掉比回退安全。
//
// `trail` 为什么不用记：撤销恢复的是上一版的原始内容，不是一次新动作。
// 往流水里塞一笔「撤销」反而污染 `trail` 这个"发生过什么"的事实源。

import type { TodoItem } from './types'

/**
 * 撤销上限。清单几十条，深拷贝一份快照在几十 KB 量级，20 步内存可忽略。
 * 定这个数不是为了省内存，是**让人有个"还能退回去多远"的心理预期**。
 */
export const UNDO_LIMIT = 20

/**
 * 深一层拷贝。改草稿不能动到内存里那份 —— 写入失败时 UI 必须纹丝不动。
 *
 * 五个引用字段得逐个复制：`{...it}` 只做浅拷贝，`trail` / `files` 这些
 * 还是指向同一个数组，改一处两边一起变（撤销栈里那快照就废了）。
 */
export function cloneItems(items: readonly TodoItem[]): TodoItem[] {
  return items.map((it) => ({
    ...it,
    origin: { ...it.origin },
    reviews: [...it.reviews],
    trail: [...it.trail],
    relates: [...it.relates],
    files: [...it.files],
  }))
}

/** 两份清单内容是否完全一致。只在"要不要压栈"时用，低频，不必优化 */
export function sameItems(a: readonly TodoItem[], b: readonly TodoItem[]): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

export interface UndoEntry {
  /** 人话，说明这一步撤的是什么（「移出列表」「改状态」…），按钮上要显示 */
  label: string
  /** **改动前**的整份清单快照 */
  items: TodoItem[]
}

export interface UndoStack {
  /**
   * 压一步。`before` 是改动前的清单（会被深拷贝一份存起来）。
   * **与栈顶内容一致就不重复压** —— 否则会出现"按了 Ctrl+Z 界面没反应"的错觉
   * （那一步本来就没改动任何东西）。
   */
  push(before: readonly TodoItem[], label: string): void
  /** 看栈顶但不出栈 */
  peek(): UndoEntry | null
  /** 出栈 */
  pop(): UndoEntry | null
  /** 清空（外部改动 / 写冲突之后必须调） */
  clear(): void
  size(): number
  /** 栈顶那步的人话说明；空栈返回空串 */
  label(): string
}

/** 造一个撤销栈。上限可传，默认 `UNDO_LIMIT` */
export function makeUndoStack(limit: number = UNDO_LIMIT): UndoStack {
  let entries: UndoEntry[] = []

  return {
    push(before, label) {
      const top = entries[entries.length - 1]
      if (top && sameItems(top.items, before)) return
      entries.push({ label: label.trim() || '改动', items: cloneItems(before) })
      if (entries.length > limit) entries = entries.slice(entries.length - limit)
    },
    peek() {
      return entries[entries.length - 1] ?? null
    },
    pop() {
      return entries.pop() ?? null
    },
    clear() {
      entries = []
    },
    size() {
      return entries.length
    },
    label() {
      return entries[entries.length - 1]?.label ?? ''
    },
  }
}
