// 卡片展示用的提取件：概要、产出文件、正文渲染。
//
// 这里的取舍一句话：**宁可少显示，不可显示错的**。

import { LIB_ROOT } from './constants'
import type { TodoItem } from './types'

/**
 * 卡片上要显示的「概要」。
 *
 * 优先 `summary`（交活时写的那一句结论）；**没写就退回 `detail` 的第一段**。
 * 这个兜底是必须的：`summary` 是后加的字段，之前那几十条待办一个都没有。
 *
 * 取第一段的规则：以**空行**为界。`detail` 是只追加的工作底稿，段落之间靠空行分隔，
 * 所以第一段通常就是"这条现在是什么状况"。没有空行时退回第一个非空行。
 */
export function briefOf(item: TodoItem): string {
  // 对缺字段要宽容：这个函数是纯的，不能把"上游补过字段"当成前提
  const summary = (item.summary ?? '').trim()
  if (summary.length > 0) return summary
  const detail = (item.detail ?? '').trim()
  if (detail.length === 0) return ''
  // 必须用 `\n{2,}` 划段，且不能写成 `\s*\r?\n\s*\r?\n\s*` ——
  // `\s` 含 `\n`，那样会把 `A\n\nB` 整体吃掉，再被末尾的 `\s*` 删掉换行。
  const firstPara = detail.split(/\n{2,}/)[0] ?? ''
  const text = firstPara.includes('\n')
    ? firstPara.split('\n').map((line) => line.trim()).find((line) => line.length > 0) ?? ''
    : firstPara.trim()
  // 兜底来的概要不是给人读的句子，可能很长 —— 掐在 200 字
  return text.length > 200 ? `${text.slice(0, 200)}…` : text
}

/**
 * 卡片上「产出」那一行显示的文件。
 *
 * **只认 `files` 字段**，不去正文里正则捞路径 —— 看过真实数据后改的：
 * 正文里的绝对路径**绝大多数不是产出**，是参考素材、粘贴的命令、报错信息。
 * 捞出来摆在「产出」下面是**误导** —— 会以为那是交付物。
 * 于是取舍变成：宁可少显示，不可显示错的。
 */
export function artifactPaths(item: TodoItem): string[] {
  const found: string[] = []
  const push = (raw: string): void => {
    const cleaned = raw.trim()
    if (cleaned.length === 0) return
    if (!found.includes(cleaned)) found.push(cleaned)
  }
  for (const file of item.files ?? []) {
    if (typeof file === 'string') push(file)
  }
  return found
}

/**
 * 把 `files` 里的相对路径还原成绝对路径（以库根为基准）。
 *
 * 绝对路径原样返回 —— 别去拼，拼了会变成 `X:\某库\X:\...`。
 *
 * **库根为空时返回空串**（不是"拼一个出来"）：库根来自配置，
 * 没配的时候相对路径根本无从还原。返回空串让调用方走"打不开"的提示 ——
 * 失败得清楚，好过打开一个用户没预期的地方。
 */
export function absolutePath(file: string, libRoot: string = LIB_ROOT): string {
  if (/^[A-Za-z]:[\\/]/.test(file)) return file
  if (libRoot.trim().length === 0) return ''
  const rel = file.replace(/^[\\/]+/, '')
  return `${libRoot}\\${rel.replace(/\//g, '\\')}`
}

/** 路径尾巴上的名字：文件要文件名，目录要目录名（末尾斜杠不算数） */
export function baseName(path: string): string {
  const parts = path.replace(/[\\/]+$/, '').split(/[\\/]/)
  return parts[parts.length - 1] ?? path
}

// ── 正文渲染：安全 markdown 子集 ───────────────────────────────────────────

export type DetailToken =
  | { t: 'text'; v: string }
  | { t: 'code'; v: string }
  | { t: 'bold'; v: string }
  | { t: 'italic'; v: string }
  | { t: 'link'; href: string; text: string }

function isSafeUrl(href: string): boolean {
  return /^(https?:|mailto:)/i.test(href)
}

/**
 * 把正文解析成 token 序列，交给模板用 `v-for` 渲染。
 *
 * 支持：`行内 code`、**粗体**、*斜体*、[text](url)、裸 https?:// 链接。
 * 其余一律当纯文本 —— **绝不用 v-html**，正文里有什么都不该变成能执行的 HTML。
 */
export function parseDetail(text: string, depth = 0): DetailToken[] {
  const out: DetailToken[] = []
  let rest = text
  while (rest.length > 0) {
    const code = /^`([^`]+)`/.exec(rest)
    if (code !== null) {
      out.push({ t: 'code', v: code[1] ?? '' })
      rest = rest.slice(code[0].length)
      continue
    }
    const link = /^\[([^\]]+)\]\(([^)\s]+)\)/.exec(rest)
    if (link !== null) {
      const href = link[2] ?? ''
      if (isSafeUrl(href)) out.push({ t: 'link', href, text: link[1] ?? '' })
      else out.push({ t: 'text', v: link[0] })
      rest = rest.slice(link[0].length)
      continue
    }
    const bold = /^\*\*([^*]+)\*\*/.exec(rest)
    if (bold !== null) {
      out.push({ t: 'bold', v: bold[1] ?? '' })
      rest = rest.slice(bold[0].length)
      continue
    }
    const italic = /^\*([^*]+)\*/.exec(rest)
    if (italic !== null) {
      out.push({ t: 'italic', v: italic[1] ?? '' })
      rest = rest.slice(italic[0].length)
      continue
    }
    const url = /^https?:\/\/[^\s<>"')\]]+/.exec(rest)
    if (url !== null) {
      const href = url[0]
      out.push({ t: 'link', href, text: href })
      rest = rest.slice(href.length)
      continue
    }
    // 一直取到下一个特殊字符；一个字符都没匹配上就吐一个字符再继续
    // （避免 `*A` 这种未闭合标记把循环卡死）
    const nextSpecial = rest.search(/(`|\*|\[|https?:\/\/)/)
    if (nextSpecial <= 0) {
      out.push({ t: 'text', v: rest.slice(0, 1) })
      rest = rest.slice(1)
    } else {
      out.push({ t: 'text', v: rest.slice(0, nextSpecial) })
      rest = rest.slice(nextSpecial)
    }
  }
  return out
}
