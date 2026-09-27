// 待办清单状态：加载、写回、冲突处理。
//
// 文件是真相源，内存只是它的一份快照。所以：
//   - 每次写都带上"这次改动所基于的指纹"，盘上不是这一份就拒绝；
//   - 被拒绝时**用盘上的内容重新加载**，绝不硬覆盖别人的改动；
//   - 文件被外部改动（AI）时整份重载。

import { reactive } from 'vue'
import { tauriApi } from '../api/tauri'
import { normalizeItem } from '../core/normalize'
import { auditItems, reconcileDoneState } from '../core/rules'
import { cloneItems, makeUndoStack } from '../core/undo'
import type { TodoItem } from '../core/types'

export const todoStore = reactive({
  items: [] as TodoItem[],
  /** 当前内存快照对应的文件内容指纹 */
  fingerprint: '',
  loading: true,
  error: '',
  /** 最近一次写入是否被冲突挡下（UI 用它弹提示） */
  conflicted: false,
})

/** 与插件一致的落盘格式（缩进 2 空格） */
function serialize(items: readonly TodoItem[]): string {
  return JSON.stringify(
    { version: 2, updatedAt: new Date().toISOString(), items },
    null,
    2,
  )
}

/** 把文件原文解析成清单（解析失败保持原样，绝不清空内存里的清单） */
function parseItems(text: string): TodoItem[] | null {
  try {
    const parsed: unknown = JSON.parse(text)
    if (typeof parsed !== 'object' || parsed === null) return null
    const raw = (parsed as { items?: unknown }).items
    if (!Array.isArray(raw)) return null
    return raw
      .map(normalizeItem)
      .filter((x): x is TodoItem => x !== null)
  } catch {
    return null
  }
}

// ── 撤销栈 ────────────────────────────────────────────────────────────────
//
// **只在 `commit()` 成功之后压栈**，并且**外部改动一发生就清栈**。
// 后者是硬的：`待办.json` 三方共写，撤销的语义是"回到上一版"，
// 而盘上被 AI 改过之后，栈里那份快照就不再是"上一版"而是"另一条线上过期的状态" ——
// 照它写回等于整份覆盖 AI 的改动，正是 `todo_io` 指纹比对要拦的事。
// 冲突时栈同样已失效，清掉比回退安全。

const undoStack = makeUndoStack()

/** 给界面看的撤销状态（栈里有几步、上一步是什么） */
export const undoState = reactive({ size: 0, label: '' })

function syncUndo(): void {
  undoState.size = undoStack.size()
  undoState.label = undoStack.label()
}

function clearUndo(): void {
  undoStack.clear()
  syncUndo()
}

/** 写回：成功则更新内存与指纹；被冲突挡下则用盘上内容重载 */
async function writeItems(items: TodoItem[], baseFingerprint: string): Promise<boolean> {
  const result = await tauriApi.todoWrite(serialize(items), baseFingerprint)
  if (result.conflict) {
    todoStore.conflicted = true
    if (result.latest !== null) {
      const parsed = parseItems(result.latest)
      if (parsed !== null) todoStore.items = parsed
    }
    todoStore.fingerprint = result.fingerprint
    // 盘上已经不是我改动所基于的那一版 → 撤销栈里的"上一版"失去了参照，全废
    clearUndo()
    return false
  }
  todoStore.conflicted = false
  todoStore.items = items
  todoStore.fingerprint = result.fingerprint
  return true
}

export async function loadTodos(): Promise<void> {
  todoStore.loading = true
  todoStore.error = ''
  // 整份重载 = 内存里那份的来路变了（外部改动 / 恢复备份），栈里的快照不再可信
  clearUndo()
  try {
    const snap = await tauriApi.todoRead()
    if (!snap.exists) {
      todoStore.items = []
      todoStore.fingerprint = snap.fingerprint
      return
    }
    const items = parseItems(snap.text)
    if (items === null) {
      // 解析失败：不清空内存、不写文件，只报错 —— 文件本身是好的，别动它
      todoStore.error = '待办文件解析失败，已保留上次载入的清单（文件未改动）'
      return
    }
    todoStore.items = items
    todoStore.fingerprint = snap.fingerprint

    // 完成态自相矛盾的记录就地修正并写回（与插件同口径：幂等，补 doneAt 用 createdAt）
    const { fixed } = reconcileDoneState(items)
    if (fixed.length > 0) {
      console.warn(`[小刀工作台] 修正 ${fixed.length} 条完成态自相矛盾的记录：${fixed.join(', ')}`)
      await writeItems(items, snap.fingerprint)
    }
    const problems = auditItems(items)
    if (problems.length > 0) {
      console.warn(`[小刀工作台] 数据自检发现 ${problems.length} 处异常：\n  - ${problems.join('\n  - ')}`)
    }
  } catch (e) {
    todoStore.error = String(e)
  } finally {
    todoStore.loading = false
  }
}

/**
 * 改动的唯一入口：给一份 draft，改完由这里统一写回。
 *
 * 为什么走这个口子：写盘可能失败（冲突、被占用），失败时内存里的清单必须纹丝不动，
 * 否则 UI 已经"看起来改好了"、盘上却没改，下次重载又跳回去 —— 那种错觉最要命。
 *
 * `label` 是给撤销按钮用的人话（「移出列表」「状态改为「进行中」」）。
 * **压栈必须压在改动之前**，所以先拷一份 `before` 出来。
 */
export async function commit(
  mutate: (draft: TodoItem[]) => void,
  label = '改动',
): Promise<boolean> {
  const before = cloneItems(todoStore.items)
  const draft = cloneItems(todoStore.items)
  mutate(draft)
  const ok = await writeItems(draft, todoStore.fingerprint)
  // 失败（写冲突）就什么都不压 —— 那一步根本没落盘，不该出现在"可撤销"里
  if (ok) {
    undoStack.push(before, label)
    syncUndo()
  }
  return ok
}

/**
 * 撤销一步：按当前指纹把上一版写回去。
 *
 * 返回 `null` 表示成功，否则是一句给用户看的话。
 *
 * **不走 `commit`**：撤销恢复的是"上一版的原始内容"，不是一次新的业务动作 ——
 * 走 `commit` 会顺手压一份新快照（等于撤销完还能"撤销撤销"，语义就乱了），
 * 也会经过 mutate 那套规则。
 *
 * 也**不记 trail**：恢复原始内容不是"发生了什么"，往流水里塞一笔反而污染事实源。
 */
export async function undo(): Promise<string | null> {
  const entry = undoStack.pop()
  if (entry === null) return '没有可撤销的操作'
  // 传一份拷贝：写入成功后它会被挂到 `todoStore.items` 上，别让栈里的那份被后续改动牵连
  const ok = await writeItems(cloneItems(entry.items), todoStore.fingerprint)
  syncUndo()
  return ok ? null : '盘上已被外部改动，这一次撤销失效了，已按最新内容刷新'
}

/** 按 id 找一条（返回引用，只读用途；要改请用 commit） */
export function findItem(id: string): TodoItem | undefined {
  return todoStore.items.find((it) => it.id === id)
}
