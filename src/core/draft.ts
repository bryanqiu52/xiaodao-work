// 新建 / 编辑表单的草稿结构。
// 与 TodoItem 分开：草稿里期限是 `YYYY-MM-DDTHH:mm` 的字符串（喂给 input），
// 落到 TodoItem 才变成 ISO 或 null。这个转换只在 draftToPatch / draftToInput 里做一次。

import { DEFAULT_DOMAIN, DEFAULT_PRIORITY, DEFAULT_STATUS } from './constants'
import type { PatchArgs } from './patch'
import type { Domain, Owner, Priority, Status } from './types'

export interface DraftFields {
  title: string
  detail: string
  summary: string
  domain: Domain
  owner: Owner
  status: Status
  priority: Priority
  /**
   * `YYYY-MM-DDTHH:mm`。**默认填"此刻"，但默认不生效** —— 见 dueOn。
   * 输入框里始终有个值（不勾也显示此刻），省得勾一下先看见一片空白。
   */
  dueAt: string
  /**
   * 这条待办**到底有没有期限**。
   * 不能拿 `dueAt !== ''` 代替：输入框里一直有值（默认此刻），
   * 那就分不清"有期限、刚好是现在"和"没期限、只是显示着默认值"。
   */
  dueOn: boolean
  link: string
  /**
   * 产出文件 / 目录。**目录末尾带 `/`**；库内文件存**相对库根**的路径
   * （分隔符一律正斜杠 `/`）—— 这个口径是跟 AI 侧 `--files` 对齐的，
   * 两边写同一个字段，格式必须一致，否则同一条待办上会一半相对一半绝对。
   */
  files: string[]
  /** 关联待办 id。短编号（`imp-046`）和长 id 都认，渲染时走 `shortId` */
  relates: string[]
}

/** 此刻的 `YYYY-MM-DDTHH:mm`（本地时区）—— `<input type="datetime-local">` 只认这个格式 */
export function nowLocalMinute(): string {
  const d = new Date()
  const mo = String(d.getMonth() + 1).padStart(2, '0')
  const da = String(d.getDate()).padStart(2, '0')
  const h = String(d.getHours()).padStart(2, '0')
  const mi = String(d.getMinutes()).padStart(2, '0')
  return `${d.getFullYear()}-${mo}-${da}T${h}:${mi}`
}

export function emptyDraft(): DraftFields {
  return {
    title: '',
    detail: '',
    summary: '',
    domain: DEFAULT_DOMAIN,
    owner: 'user',
    status: DEFAULT_STATUS,
    priority: DEFAULT_PRIORITY,
    dueAt: nowLocalMinute(),
    dueOn: false,
    link: '',
    files: [],
    relates: [],
  }
}

export function draftFromItem(item: {
  title: string
  detail: string
  summary: string
  domain: Domain
  owner: Owner
  status: Status
  priority: Priority
  dueAt: string | null
  link: string
  files?: string[]
  relates?: string[]
}): DraftFields {
  return {
    title: item.title,
    detail: item.detail,
    summary: item.summary,
    domain: item.domain,
    owner: item.owner,
    status: item.status,
    priority: item.priority,
    // `<input type="datetime-local">` 只认到分钟
    dueAt: item.dueAt ? item.dueAt.slice(0, 16) : nowLocalMinute(),
    // 有期限才勾上；本来没期限的仍是"不勾 = 没有期限"
    dueOn: item.dueAt !== null && item.dueAt.length > 0,
    link: item.link,
    // 拷一份，别直接引用 item 上那个数组 —— 草稿改了不该动内存里的清单
    files: [...(item.files ?? [])],
    relates: [...(item.relates ?? [])],
  }
}

/** 草稿 → applyPatch 的参数。没勾期限就传空串（= 把期限清掉） */
export function draftToPatch(d: DraftFields): PatchArgs {
  return {
    title: d.title,
    detail: d.detail,
    summary: d.summary,
    domain: d.domain,
    owner: d.owner,
    status: d.status,
    priority: d.priority,
    due: d.dueOn ? d.dueAt : '',
    link: d.link,
    files: d.files,
    relates: d.relates,
  }
}

/** 草稿 → 新建参数 */
export function draftToInput(d: DraftFields): {
  title: string
  domain: Domain
  owner: Owner
  status: Status
  priority: Priority
  detail: string
  summary: string
  dueAt: string | null
  link: string
  files: string[]
  relates: string[]
} {
  return {
    title: d.title,
    domain: d.domain,
    owner: d.owner,
    status: d.status,
    priority: d.priority,
    detail: d.detail,
    summary: d.summary,
    dueAt: d.dueOn && d.dueAt.length > 0 ? new Date(d.dueAt).toISOString() : null,
    link: d.link,
    files: d.files,
    relates: d.relates,
  }
}
