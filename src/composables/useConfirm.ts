// 确认浮层的**唯一入口**。
//
// **为什么不用系统原生 `confirm()`**：那个是 Windows 自己的消息框 —— 白底、系统字体，
// 和这套玻璃拟态界面完全是两个世界；而且它**不认 markdown**，我们写在里面的
// `**所有数据**` 会连着星号一起显示出来（真这么显示了好久）。
// 2026-09-26 把五处（恢复备份 / 初始化 / 换数据目录 / 恢复默认目录 / 删流水）
// 统一换成了应用内的浮层。
//
// **为什么做成"promise + 全局单例"而不是一个到处传的组件**：
// 调用点想要的就是 `if (!(await askConfirm({...}))) return` 这种一句话写法。
// 组件式的写法（`<Confirm v-if="...">` + 几个中间状态）会在每个调用点摊出三四个变量，
// 五处就是五份重复。这里用「一个全局状态 + 一个常驻宿主组件（`ConfirmSheet.vue`）」，
// 调用点只留"问什么、答了怎么办"。

import { reactive } from 'vue'

export interface ConfirmOptions {
  /** 标题。**写成一个问题**（"用这份备份覆盖当前清单？"），一眼知道在问什么 */
  title: string
  /** 说明段落，每段一句 */
  desc?: string[]
  /** **原样复述**的东西：路径、备份名、被删的那条 —— 单独一块灰底，好认 */
  note?: string
  /** 逐条要点（"会发生什么"），前面自动带点 */
  facts?: string[]
  /** 底部提醒，突出显示。危险操作的最后一道栏杆：写清"能不能后悔" */
  warning?: string
  confirmText?: string
  cancelText?: string
  /** 破坏性操作：标题带警示图标、确认按钮走红色 */
  danger?: boolean
}

interface ConfirmState {
  open: boolean
  opts: ConfirmOptions | null
}

export const confirmState = reactive<ConfirmState>({ open: false, opts: null })

/** 当前这次询问的结算回调。null = 没人等着（浮层没开） */
let resolver: ((ok: boolean) => void) | null = null

/**
 * 问一句，等答复。`true` = 用户点了确认。
 *
 * **先把上一次结算掉**：万一上一句还没答完就来了新的一句（连点两下按钮、
 * 或某个流程里嵌了一次询问），不结算的话那个 `await` 会**永远挂着** ——
 * 调用方后面那些代码再也不执行，而且**不报错**。
 * 按"取消"结算是安全的那一侧。
 */
export function askConfirm(opts: ConfirmOptions): Promise<boolean> {
  settle(false)
  confirmState.opts = opts
  confirmState.open = true
  return new Promise<boolean>((resolve) => {
    resolver = resolve
  })
}

/** 结算并关掉。由 `ConfirmSheet.vue` 调，业务代码别碰 */
export function settle(ok: boolean): void {
  const done = resolver
  resolver = null
  confirmState.open = false
  confirmState.opts = null
  done?.(ok)
}
