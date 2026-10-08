// 配置契约：与 Rust `config.rs` 的 AppConfig **字段一一对应**。
//
// 两边都靠 `#[serde(default)]` / 默认值兜底，所以字段增减时只要两边一起改就行，
// 老配置文件缺字段也不会起不来。

import { DEFAULT_SORT } from './constants'

export interface WindowState {
  width: number
  height: number
  x: number | null
  y: number | null
}

export interface AppConfig {
  /** light / dark / system */
  theme_mode: string
  /**
   * 浅色主题下的强调色 hex；空串 = 用浅色内置那支（蓝 #4176E6）。
   *
   * **深浅各存一支**：一支颜色伺候不了两个底色 —— 暗色下好看的荧光绿放到白底上
   * 会糊成一片，反过来亮蓝压在深底上又发闷（2026-09-25 起分两套）。
   */
  accent_light: string
  /** 深色主题下的强调色 hex；空串 = 用暗色内置那支（荧光绿 #C8FF3E） */
  accent_dark: string
  /**
   * 旧字段：深浅共用一支强调色（1.0.2 起不再使用）。
   * 留着只为**迁移**：老配置文件里这个值还在，程序启动时会搬给 accent_light 再清空。
   */
  accent_color: string
  /** 界面字号百分比（85–200，默认 100；只缩放字号与行高，间距不动） */
  ui_scale: number
  /**
   * 卡片/面板背景的不透明度（0.4–1.0，默认 1 = 不透明）。
   * 调小能透出背景和壁纸 —— 值乘进卡面底色的 alpha 里，不是整窗口透明。
   */
  glass_opacity: number
  /** 窗口四角圆角（0–24 逻辑像素，默认 16）。0 = 直角 */
  window_radius: number
  /**
   * 卡片模糊度（0–40 px，默认 0 = 不糊）：卡片背后的东西被糊多少。
   * 调低「卡片不透明度」之后才看得出来；沉浸模式下至少 16px（那是沉浸的定义）。
   */
  card_blur: number
  /** 壁纸图片路径（空 = 不启用）。选图后会被拷进数据根，用户挪走原图也不失效 */
  wallpaper_path: string
  /** 壁纸蒙版浓度（0–0.85）：压暗壁纸，保证卡片上的字看得清 */
  wallpaper_veil: number
  /** 壁纸整体模糊（沉浸模式下自动停用，交给卡片的毛玻璃去糊） */
  wallpaper_blur: boolean
  /** 沉浸模式：卡片变真毛玻璃，透出壁纸的模糊轮廓 */
  wallpaper_immersive: boolean
  /** 壁纸是否覆盖到顶部标题栏（关掉则标题栏保留自己的底色） */
  wallpaper_header: boolean

  global_shortcut: string
  /** 关窗行为：hide（藏到托盘）/ quit（退出） */
  close_behavior: string
  always_on_top: boolean
  /** 贴边隐藏：拖到屏幕左右边缘停住后自动收起，鼠标划到边缘再滑出 */
  edge_hide: boolean
  window: WindowState

  /**
   * 上一次「复制位置说明」时用过的**数据根**。
   *
   * 留着它是为了**能说出"从哪变到哪"** —— 只报当前值的话，AI 那边没法判断
   * 是不是就是它记着的那份。空串 = 从没告知过（那就只说当前是什么）。
   *
   * 待办文件的路径**不单独存**：它 = `数据根\待办.json`，
   * 旧路径拿这份旧数据根现推就行，不用再养一份容易对不上的副本。
   */
  notified_data_root: string
  /** 库根：打开产出的根目录 */
  library_root: string
  /** 默认视图：user / agent */
  default_view: string
  /**
   * 待办列表的排序口径：priority（重要程度）/ recent（最近更新）。
   *
   * **为什么存下来**：这是"我现在想怎么看"的偏好，不是一次性的动作 ——
   * 每次开都得重切一遍的话，等于没这个功能。跟 `default_view` 一个道理。
   */
  todo_sort: string
  show_deleted: boolean

  /** md 打开方式：preview / system */
  md_open_mode: string
  preview_exe: string
  open_roots: string[]

  backup_enabled: boolean
  backup_keep: number

  /** 到期提醒总开关 */
  remind_enabled: boolean
  /** 提前几天提醒（0 = 当天才提醒） */
  remind_advance_days: number
  /** 最近一次提醒的日期（YYYY-MM-DD）；跟今天对不上就作废当天记录 */
  remind_day: string
  /** 当天已提醒过的条目 id（只记当天、不留历史） */
  remind_done: string[]
  /** 被「稍后再说」推迟的条目 id */
  remind_snoozed: string[]
  /** 推迟到什么时候（毫秒时间戳；0 = 没有推迟） */
  remind_snooze_until: number

  // ---- 番茄闹钟 ----
  /** 专注时长（分钟） */
  focus_work_min: number
  /** 短休时长（分钟） */
  focus_short_min: number
  /** 长休时长（分钟） */
  focus_long_min: number
  /** 长休前的专注轮数（专注 × N 轮后接长休） */
  focus_rounds: number
  /** 阶段结束后自动开始下一阶段（不勾则停下等用户点开始） */
  focus_auto_continue: boolean
  /** 到点提示音（系统通知始终发，这里只管响不响） */
  focus_sound: boolean
  /** 今日完成番茄数的日期（YYYY-MM-DD）；跟今天对不上就清零 */
  focus_done_day: string
  /** 今日完成的番茄数（只记当天，跨天重置） */
  focus_done_count: number

  // ---- 领域分类（可编辑）----
  // 内置那几类是通用分法，换个人多半还要调，所以做成可编辑：
  // 空数组 = 用内置默认；填了就完全按填的来（含界面选项、筛选、以及 AI 侧的校验）。
  /** 领域分类的 key 列表；空数组 = 用内置默认 */
  domains: string[]
  /** 分类的中文名；没配到的 key 回落内置标签 */
  domain_labels: Record<string, string>

  /** 首次启动向导看过了没（看过了就不再弹） */
  welcome_done: boolean

  /**
   * 上次查更新的**本地日期**（`YYYY-MM-DD`）。空串 = 从没查过。
   *
   * 用来做"**一天最多查一次**"的节流 —— 每次启动都去打一次 GitHub 是不礼貌的，
   * 匿名接口还有速率限制，撞上限流反而查不到。
   */
  update_checked_at: string
  /**
   * 用户说了"这一版先不更新"的版本号。
   *
   * 弹过一次就不再弹**这一版**，免得每次启动都拿同一件事烦人；
   * 但出现更新的版本时要弹（那说明又有新东西了）。
   */
  update_skipped_version: string
}

export function defaultConfig(): AppConfig {
  return {
    // 默认暗色：这套配色就是照着原插件暗色那套调出来的，亮色是备选
    theme_mode: 'dark',
    accent_light: '',
    accent_dark: '',
    accent_color: '',
    ui_scale: 100,
    glass_opacity: 1,
    window_radius: RADIUS_DEFAULT,
    // 默认不糊：给每张卡片都开一层模糊是实打实的合成开销，要的人自己开
    card_blur: 0,
    wallpaper_path: '',
    wallpaper_veil: 0.3,
    wallpaper_blur: true,
    wallpaper_immersive: false,
    wallpaper_header: true,
    global_shortcut: 'Ctrl+Shift+Space',
    close_behavior: 'hide',
    always_on_top: false,
    edge_hide: true,
    window: { width: 450, height: 980, x: null, y: null },
    // 下面这几项**运行时以后端 config.json 为准**（每次启动它都会带完整配置回来），
    // 这里留空是故意的：以前写的是作者本机的绝对路径，哪条路径漏了兜底，
    // 界面上就会冒出一个别人机器上根本不存在的盘符
    notified_data_root: '',
    library_root: '',
    default_view: 'user',
    todo_sort: DEFAULT_SORT,
    show_deleted: false,
    md_open_mode: 'preview',
    preview_exe: '',
    open_roots: [],
    backup_enabled: true,
    backup_keep: 14,
    remind_enabled: true,
    // 默认提前一天：当天才提醒往往已经来不及准备了
    remind_advance_days: 1,
    remind_day: '',
    remind_done: [],
    remind_snoozed: [],
    remind_snooze_until: 0,
    focus_work_min: 25,
    focus_short_min: 5,
    focus_long_min: 15,
    focus_rounds: 4,
    // 自动连播默认关：市面主流也默认手动，怕休息被跳过
    focus_auto_continue: false,
    focus_sound: true,
    focus_done_day: '',
    focus_done_count: 0,
    // 空 = 用内置那 6 类（别人可以在设置里换成自己的）
    domains: [],
    domain_labels: {},
    welcome_done: false,
    // 空串 = 从没查过 / 没有"跳过"的版本，所以第一次启动会自动查一次更新
    update_checked_at: '',
    update_skipped_version: '',
  }
}

/** 字号缩放的安全区间：再小看不清，再大窄窗口里会挤成一团（超过 150% 建议把窗口拉宽） */
export const UI_SCALE_MIN = 85
export const UI_SCALE_MAX = 200

/** 卡片不透明度的区间：再低卡片就成一片雾，字读不清了 */
export const GLASS_MIN = 0.4
export const GLASS_MAX = 1

/** 卡片模糊度的区间（px）：0 = 不糊，再大就糊成一团看不出背后是什么了 */
export const CARD_BLUR_MIN = 0
export const CARD_BLUR_MAX = 40

export function clampCardBlur(v: number): number {
  if (!Number.isFinite(v)) return CARD_BLUR_MIN
  return Math.min(CARD_BLUR_MAX, Math.max(CARD_BLUR_MIN, Math.round(v)))
}
/**
 * 窗口圆角的区间。
 * 上限 50：窗口宽 450，50px 圆角约合"宽度的 11%" —— 跟 iPhone 屏幕圆角占屏宽的比例
 * （约 14%）是同一量级，再往上就开始像个胶囊、横向内容也会被圆角挤到。
 */
export const RADIUS_MIN = 0
export const RADIUS_MAX = 50
/** 窗口圆角默认值：对齐 iOS 的"大卡片"档，也是最初定下的观感 */
export const RADIUS_DEFAULT = 16
/** 圆角一次调一档，对齐到 2 的倍数 —— 1px 的差别看不出来，只会让滑杆很吵 */
export const RADIUS_STEP = 2
/** 壁纸蒙版的区间：再高超标就压成一片纯色，壁纸白选了 */
export const VEIL_MAX = 0.85

/** 夹回安全区间（配置可能被手改坏，不能让界面变成不可读） */
export function clampGlass(v: number): number {
  if (!Number.isFinite(v)) return 1
  return Math.min(GLASS_MAX, Math.max(GLASS_MIN, v))
}

export function clampVeil(v: number): number {
  if (!Number.isFinite(v)) return 0.3
  return Math.min(VEIL_MAX, Math.max(0, v))
}

/** 夹回区间并对齐到 2 的倍数（避免 17 这种没有意义的取值） */
export function clampWindowRadius(v: number): number {
  if (!Number.isFinite(v)) return RADIUS_DEFAULT
  const snapped = Math.round(v / RADIUS_STEP) * RADIUS_STEP
  return Math.min(RADIUS_MAX, Math.max(RADIUS_MIN, snapped))
}

/** 把百分比夹回安全区间，并对齐到 5 的倍数（避免 137% 这种没有意义的取值） */
export function clampUiScale(v: number): number {
  if (!Number.isFinite(v)) return 100
  const snapped = Math.round(v / 5) * 5
  return Math.min(UI_SCALE_MAX, Math.max(UI_SCALE_MIN, snapped))
}

/** 主题的实际取值：把 system 解析成 light / dark */
export function resolveTheme(mode: string): 'light' | 'dark' {
  if (mode === 'light' || mode === 'dark') return mode
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }
  return 'light'
}
