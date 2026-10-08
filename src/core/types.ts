// 待办的数据契约 —— 与 AI 侧（xiaodao-work skill）**逐字段一致**，两边读写同一份 json。
//
// 唯一的变化：插件主机端为了塞进工具输出的 `Record<string, JsonValue>` 约束，
// 给 TodoReview / TodoTrail 加了 `[key: string]: string` 索引签名。那是工具约束的产物，
// 不是数据的一部分，桌面端没有工具系统，去掉。

/**
 * 领域分类。**现在是开放的**。
 *
 * 2026-09-24 从固定联合类型放开成 `string`：分类改成设置里可编辑之后，
 * 用户完全可以换成自己的一套（"客户 / 项目 / 学习"之类），写死六个英文值
 * 就名不副实了 —— 每加一个自定义分类都要断言一次，说明类型本身错了。
 * 内置那几个只是默认值，见 `core/constants.ts` 的 `DOMAIN_OPTIONS`。
 */
export type Domain = string
export type Owner = 'user' | 'agent'
export type Status = 'todo' | 'progress' | 'paused' | 'waiting' | 'done'
export type Priority = 'high' | 'mid' | 'low'

export type ReviewAction = 'approved' | 'rejected' | 'shelved'
export type TrailKind = 'create' | 'status' | 'edit' | 'review' | 'transfer' | 'comment'

/** 审批留痕：只记评审表态 */
export interface TodoReview {
  action: ReviewAction
  at: string
  comment: string
}

/** 动作流水：全部动作，只增不改。text 是"人话"，直接可读 */
export interface TodoTrail {
  kind: TrailKind
  at: string
  text: string
  /** 来源会话 id（桌面端拿不到，留空） */
  by: string
}

/** 脉络：from 是人话来源，不是 session id */
export interface TodoOrigin {
  from: string
  why: string
}

export interface TodoItem {
  /** 主键：自动生成 `i<base36>-<6位随机>`，也允许语义编号（如 `imp-062`） */
  id: string
  title: string
  /** 工作底稿：只追加不覆盖 */
  detail: string
  /** 汇报概要（一句话结论）；空串时展示层回退 detail 第一段 */
  summary: string
  domain: Domain
  owner: Owner
  status: Status
  priority: Priority
  dueAt: string | null
  createdAt: string
  /** 硬不变式：`(doneAt !== null) <=> (status === 'done')` */
  doneAt: string | null
  /** 软删除：非 null = 已移出列表，但记录留在文件里 */
  deletedAt: string | null
  reviews: TodoReview[]
  trail: TodoTrail[]
  origin: TodoOrigin
  relates: string[]
  files: string[]
  source: string
  link: string
}

/** 待办文件的顶层结构 */
export interface TodoFile {
  version: number
  updatedAt: string
  items: TodoItem[]
}

/** 筛选值：null = 全部 */
export type FilterValue = string | null
