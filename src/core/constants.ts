// 领域常量：取值、中文名、排序、旧值映射。
//
// 这些值是从插件照抄的，不是重新设计的 —— 盘上几十条真数据用的就是这套取值，
// 改一个字都会让老记录的分组散掉。

import type { Domain, Owner, Priority, SortMode, Status } from './types'

// ── 领域 ──────────────────────────────────────────────────────────────────

/**
 * 领域分类的**内置默认** —— 给**纯净版（发布给别人）**用的通用 4 类。
 *
 * 原来内置的是 6 类（`client` / `company` / `content` / `plan` / `assistant` / `personal`）
 * —— 那是**按某一个人的业务分法**定的，换个人根本对不上。
 * 老机器上那份已经固化进它自己的 `config.json`，不受这次改动影响。
 *
 * 这 4 类是任何人都能立刻上手的，而且**都能删能改名**（设置 → 待办）。
 */
export const DOMAIN_OPTIONS: readonly Domain[] = ['work', 'life', 'study', 'other']

export const DOMAIN_LABELS: Record<string, string> = {
  work: '工作',
  life: '生活',
  study: '学习',
  other: '其他',
}

// ── 自定义分类（设置里可编辑）──────────────────────────────────────────────
//
// 内置那几类是通用分法，换个人多半还要改。用户在设置里改了分类之后，这里必须跟着变 ——
// 否则新分类会被 `normalizeDomain` 打回 `personal`（它只认 `DOMAIN_OPTIONS`），
// 表现就是"改了分类，但待办还是老样子"。所以留一份当前生效的分类，载入配置时刷新。
let activeDomains: readonly string[] = DOMAIN_OPTIONS
let activeLabels: Record<string, string> = {}

/** 载入配置后调用：设当前生效的分类。空列表 = 回内置那 6 类 */
export function setActiveDomains(
  keys: readonly string[],
  labels: Record<string, string> = {},
): void {
  activeDomains = keys.length > 0 ? keys : DOMAIN_OPTIONS
  activeLabels = labels ?? {}
}

/** 当前生效的分类 key 列表：界面选项、筛选、normalize 都认这个 */
export function activeDomainOptions(): readonly string[] {
  return activeDomains
}

/** 当前生效的中文名：自定义 > 内置 > 原样显示 key */
export function domainLabel(key: string): string {
  return activeLabels[key] ?? DOMAIN_LABELS[key] ?? key
}

/**
 * 认不出来的分类（旧数据、被删掉的分类）归到哪：**当前列表的第一项**。
 *
 * 不能写死 `personal` —— 分类现在是用户可配的，`personal` 未必在他的列表里，
 * 归到一个已不存在的分类，界面上就显示成光秃秃的英文 key。
 */
export function fallbackDomain(): Domain {
  return activeDomainOptions()[0] ?? DOMAIN_OPTIONS[0] ?? 'other'
}

/**
 * 旧领域 → 新领域的映射。
 * 盘上还躺着用旧口径写的记录，读盘时统一映射上来，别让它们落进"未分类"。
 */
export const LEGACY_DOMAINS: Record<string, Domain> = {
  business: 'client',
  media: 'content',
  wiki: 'assistant',
  other: 'personal',
}

export const DEFAULT_DOMAIN: Domain = 'personal'

// ── 权重（优先级）──────────────────────────────────────────────────────────

export const PRIORITY_OPTIONS: readonly Priority[] = ['high', 'mid', 'low']
export const PRIORITY_LABELS: Record<string, string> = { high: '高', mid: '中', low: '低' }
export const PRIORITY_RANK: Record<string, number> = { high: 0, mid: 1, low: 2 }
export const DEFAULT_PRIORITY: Priority = 'mid'

// ── 归属 ──────────────────────────────────────────────────────────────────

export const OWNER_USER: Owner = 'user'
export const OWNER_AGENT: Owner = 'agent'
export const OWNER_OPTIONS: readonly Owner[] = [OWNER_USER, OWNER_AGENT]
export const OWNER_LABELS: Record<string, string> = { user: '我的', agent: '小刀的' }
export const OWNER_HINTS: Record<string, string> = {
  user: '我要办的事',
  agent: '小刀自己要办的事（可以看它在忙什么、卡在哪）',
}

// ── 状态 ──────────────────────────────────────────────────────────────────

/**
 * 状态切换按钮的顺序：**暂停 / 排队排在最前** —— 这两档最常点
 * （把活挂起来、或者把小刀叫回来接着干）。
 */
export const STATUS_OPTIONS: readonly Status[] = ['paused', 'todo', 'progress', 'waiting', 'done']

/**
 * 状态**筛选标签**的顺序：按事情的自然流程排（排队 → 进行中 → 待回复 → 已暂停）。
 *
 * 为什么不复用 STATUS_OPTIONS：那个是给动作按钮排的（按点击频率），
 * 这个是给人读的（按流程）。两处口径不同，各留一份。
 * 「已完成」不在这组里 —— 它归「显示已完成」开关管。
 */
export const STATUS_FILTER_OPTIONS: readonly Status[] = ['todo', 'progress', 'waiting', 'paused']

/**
 * 状态的中文名。**两边通用，不绑死在某一个视图上。**
 *
 * `waiting` 原来叫「等你回话」—— 那是站在小刀的活上说的（球在你脚下）。
 * 可待办会在「我的」和「小刀的」之间转手，同一条到了「我的」里还写「等你回话」，
 * 就成了"你等你自己的回话"，读不通。改成「待回复」：小刀那条是它在等你，
 * 你这条是你在等别人 —— 主语交给流水里那句话去说。
 */
export const STATUS_LABELS: Record<string, string> = {
  todo: '排队',
  progress: '进行中',
  paused: '已暂停',
  waiting: '待回复',
  done: '已完成',
}

/** 状态按钮上的短文字：窄栏里也放得下 */
export const STATUS_ACTION_LABELS: Record<string, string> = {
  paused: '暂停',
  todo: '排队',
  progress: '进行中',
  waiting: '待回复',
  done: '完成',
}

/** 小刀视图的排班顺序：进行中 → 待回复 → 排队 → 已完成 → 已暂停垫底 */
export const STATUS_RANK: Record<string, number> = {
  progress: 0,
  waiting: 1,
  todo: 2,
  done: 3,
  paused: 4,
}

export const DEFAULT_STATUS: Status = 'todo'

// ── 流水类型 ──────────────────────────────────────────────────────────────

/**
 * 流水里那条是什么动作。
 *
 * 评审也走流水（`recordReview` 会顺手记一条 `review`，留言也在里面），
 * 所以脉络里**不再单独开一栏「评审」** —— 那等于把同一件事摆两遍。
 * 挂个类型标就够：一眼能挑出哪条是评审、哪条是转手、哪条是自己记的。
 *
 * `comment` 是脉络里那个「记一笔」写出来的 —— 叫「手记」是为了跟
 * 系统自己记的那些分开：这是你亲手写的。
 */
export const TRAIL_KIND_LABELS: Record<string, string> = {
  create: '新建',
  status: '状态',
  edit: '编辑',
  review: '评审',
  transfer: '移交',
  comment: '手记',
}

// ── 待办排序 ──────────────────────────────────────────────────────────────

/**
 * 默认按**重要程度**排 —— 那是"先做什么"的顺序，也是待办清单该有的样子。
 * 「最近更新」是拿来**找刚有变动的那条**的，属于临时切过去看一眼。
 */
export const DEFAULT_SORT: SortMode = 'priority'

/** 顶栏切换按钮上的短名：窗口只有 450 宽，全名放不下 */
export const SORT_SHORT: Record<string, string> = { priority: '重要', recent: '最近' }

export const SORT_LABELS: Record<string, string> = { priority: '重要程度', recent: '最近更新' }

/**
 * 认不出来的排序值一律回「重要程度」。
 *
 * 配置文件是能被手改坏的，读回来一个乱值不能让列表变成没有顺序的样子 ——
 * 宁可回到最熟悉的那套，也比"看起来随机"强。
 */
export function normalizeSort(v: string): SortMode {
  return v === 'recent' ? 'recent' : DEFAULT_SORT
}

// ── 其它 ──────────────────────────────────────────────────────────────────

/** 成功提示停留时长 */
export const FLASH_MS = 1500
/** 删除确认无人应答时自动撤回的时长 */
export const CONFIRM_MS = 5000

/**
 * 编号截断阈值：短的全留，长的截中间（`imtvb3v…qn9hs`）。
 *
 * **14 是量出来的**：截完固定占 13 字符（7 + `…` + 5），所以只有原长 > 13 才真的变短。
 * 取 12 会让 13 字符的 id 截完还是 13 字符 —— 白截还多一个省略号。
 */
export const ID_SHORT_MAX = 14

// ── 版本 ──────────────────────────────────────────────────────────────────

/**
 * 界面上显示的版本号：**标题栏**和「设置 → 关于」都读它。
 *
 * 发版时跟这三处一起改：`package.json` / `src-tauri/tauri.conf.json` /
 * `src-tauri/Cargo.toml` —— 三处都必须是三段 semver（安装包和 Windows 资源
 * 文件不认两段），这里跟它们保持一致（流程见 RELEASE.md）。
 *
 * 只留这一份：两个界面各写一份，迟早会出现"标题栏 1.3.0、关于里还是 1.2.1"。
 */
export const APP_VERSION = '1.3.0'

// ── GitHub（版本发布与更新检查）────────────────────────────────────────────

/**
 * 本项目的 GitHub 地址。**改名时只改这两行**。
 *
 * 「检查更新」读的就是它拼出来的 `releases/latest` 接口。
 * 用户名或仓库名以后要是改了，GitHub 会自动跳转旧地址 ——
 * **已经发出去的老版本照样查得到更新**，所以改名不会把老用户甩下。
 */
export const GITHUB_OWNER = 'bryanqiu52'
export const GITHUB_REPO = 'xiaodao-work'

export const GITHUB_URL = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}`

/** 某个版本的 Release 页面（用户点「去下载」跳这儿） */
export function releaseUrl(version: string): string {
  return `${GITHUB_URL}/releases/tag/v${version.replace(/^v/, '')}`
}

/**
 * 库根的**兜底值 —— 故意留空**。
 *
 * 真实的库根从配置来（`library_root`，默认就是数据根），由调用方显式传进来
 * （`ArtifactChip` / `TodoRow` 拿的都是 `configStore.cfg.library_root`）。
 * 这里**不能写死任何绝对路径**：写一个作者的路径，换台机器就指向一个不存在的盘符，
 * 而且失败得毫无线索 —— 这也是整个项目"发布版的默认值里不许出现本机路径"的一部分。
 *
 * 留空的后果很明确：相对路径没法还原成绝对路径，打开产出会失败并提示，
 * **而不是打开一个莫名其妙的地方**。宁可失败得清楚。
 */
export const LIB_ROOT = ''

/**
 * 把系统文件选择器返回的**绝对路径**转成 `files[]` 里该存的样子。
 *
 * 口径是跟 AI 侧（`xiaodao` skill）对齐的，两边写的是同一个字段：
 *   - 库内的 → **相对库根**，分隔符一律换成正斜杠 `/`（`某个目录/一些文件.md` 这种）；
 *   - 库外的 → 保留绝对路径，但分隔符也换成 `/`（真实数据里就是这么存的）；
 *   - **目录末尾补一个 `/`** —— 那是"这是个目录"的唯一标记，
 *     `artifactIcon()` 和 AI 侧的读取都靠它认，少了就当文件处理。
 *
 * 为什么坚持相对路径：更短，而且整库搬家之后引用不作废（绝对路径会全部失效）。
 *
 * 比前缀时**大小写不敏感**：Windows 路径本来就大小写不敏感，
 * 用户从盘符大小写不一致的位置选进来的路径照样该认。
 */
export function toLibraryRelative(abs: string, libraryRoot: string = LIB_ROOT): string {
  const slash = abs.trim().replace(/\\/g, '/').replace(/\/+$/, '')
  if (slash.length === 0) return ''
  // 目录标记：盘上确实是个目录时调用方会传 isDir，这里只按"末尾有没有斜杠"保底
  const root = libraryRoot.trim().replace(/\\/g, '/').replace(/\/+$/, '')
  // 允许挂库外的文件（AI 侧也允许），不因为不在库根下就拒绝
  const lowerRoot = root.toLowerCase()
  const lower = slash.toLowerCase()
  // 选中的正好是库根自己 → 没法相对化（相对路径是空串，指代不明），原样留绝对路径
  if (lower === lowerRoot) return slash
  if (lower.startsWith(`${lowerRoot}/`)) return slash.slice(root.length + 1)
  return slash
}

/** 目录路径的规范写法：末尾一定带 `/`（已是那个样子就别重复加） */
export function withDirSlash(path: string): string {
  return path.endsWith('/') ? path : `${path}/`
}

/** 查看器吃得下的扩展名（它只认 .md / .txt，别的进去是空白页） */
export const PREVIEW_EXTS: readonly string[] = ['.md', '.txt']

// ── 关联文件的图标 ────────────────────────────────────────────────────────

/**
 * 关联文件的图标：按扩展名分大类（照搬原插件的一张表）。
 *
 * **为什么用 emoji 而不是图标组件**：一排文件小签摆在一起，要能"扫一眼认出类型"——
 * 全是一张纸片的话，分不出哪个是文件夹、哪个是表格。emoji 自带颜色，这个场景正合适。
 *
 * 只挑常见几类，认不出来的回落 `📄`：把每个扩展名都列一遍既写不完，也没人看得出区别。
 */
const IMAGE_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico', 'heic', 'tif', 'tiff', 'avif']
const VIDEO_EXTS = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv', 'wmv', 'm4v']
const AUDIO_EXTS = ['mp3', 'wav', 'm4a', 'flac', 'aac', 'ogg']
const SHEET_EXTS = ['xls', 'xlsx', 'csv', 'et', 'ods']
const SLIDE_EXTS = ['ppt', 'pptx', 'key', 'odp']
const TEXT_EXTS = ['md', 'txt', 'markdown', 'log', 'rtf']
const DOC_EXTS = ['doc', 'docx', 'pages', 'odt']
const ARCHIVE_EXTS = ['zip', 'rar', '7z', 'tar', 'gz', 'tgz']
const CODE_EXTS = [
  'js', 'ts', 'tsx', 'jsx', 'mjs', 'cjs', 'json', 'html', 'css', 'py', 'java', 'go',
  'rs', 'rb', 'c', 'h', 'cpp', 'sh', 'sql', 'vue', 'yml', 'yaml',
]

export function artifactIcon(path: string): string {
  // 目录在 files 里一律带尾斜杠，先把它认出来 —— 它不走扩展名那一套
  if (/[\\/]$/.test(path)) return '📁'
  const matched = /\.([^.\\/]+)$/.exec(path.replace(/[\\/]+$/, ''))
  const ext = (matched?.[1] ?? '').toLowerCase()
  if (IMAGE_EXTS.includes(ext)) return '🖼️'
  if (VIDEO_EXTS.includes(ext)) return '🎬'
  if (AUDIO_EXTS.includes(ext)) return '🎵'
  if (ext === 'pdf') return '📕'
  if (SHEET_EXTS.includes(ext)) return '📊'
  if (SLIDE_EXTS.includes(ext)) return '📽️'
  if (TEXT_EXTS.includes(ext)) return '📝'
  if (DOC_EXTS.includes(ext)) return '📄'
  if (ARCHIVE_EXTS.includes(ext)) return '📦'
  if (CODE_EXTS.includes(ext)) return '💾'
  return '📄'
}

/**
 * 不代跑的扩展名：这些类型的默认动作是**直接执行**，
 * 点一下就跑不该由程序替用户决定 —— 命中就退化成"打开所在文件夹并选中"。
 */
export const NO_LAUNCH_EXTS: readonly string[] = [
  '.exe', '.bat', '.cmd', '.com', '.msi', '.msp', '.scr', '.ps1', '.psm1',
  '.vbs', '.vbe', '.js', '.jse', '.wsf', '.wsh', '.hta', '.lnk', '.jar',
  '.appref-ms', '.dll', '.sys', '.drv',
]
