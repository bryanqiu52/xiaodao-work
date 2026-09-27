// 配置状态：读、写、以及把主题应用到 DOM。

import { reactive } from 'vue'
import { tauriApi } from '../api/tauri'
import {
  clampCardBlur,
  clampGlass,
  clampUiScale,
  clampWindowRadius,
  defaultConfig,
  resolveTheme,
  type AppConfig,
} from '../core/config'
import { setActiveDomains } from '../core/constants'

export const configStore = reactive({
  cfg: defaultConfig(),
  loaded: false,
  dataRoot: '',
})

export async function loadConfig(): Promise<void> {
  configStore.cfg = await tauriApi.configGet()
  // 老配置的强调色只有一支，先搬进深浅两支再往下走（搬完界面上看到的还是原来那支）
  await migrateAccent()
  configStore.dataRoot = await tauriApi.dataRootPath()
  configStore.loaded = true
  // **必须在读待办之前**刷好分类：`normalizeDomain` 认的是当前生效的分类，
  // 晚一步的话，用自定义分类写的记录会被打回 personal
  syncDomains()
  applyTheme()
}

/**
 * 迁移：1.0.2 之前强调色深浅共用一支（`accent_color`）。
 *
 * **两支都填旧值** —— 老版本里那支色在两个主题下都生效，只搬给其中一支的话，
 * 用户切到另一个主题会"颜色自己变了"。搬完立刻落盘并把旧字段清空，下次不再搬。
 */
async function migrateAccent(): Promise<void> {
  const old = configStore.cfg.accent_color.trim()
  if (old.length === 0) return
  await saveConfig({ accent_light: old, accent_dark: old, accent_color: '' })
}

/**
 * 把配置里的自定义分类同步给 `core`。
 *
 * 界面选项、筛选、以及读盘时的归一化都读这份 —— 少了这一步，
 * 用户在设置里加的分类就只是"能选、存不下"（一读盘就被打回内置）。
 */
function syncDomains(): void {
  setActiveDomains(configStore.cfg.domains, configStore.cfg.domain_labels)
}

/**
 * 把主题模式、强调色、字号缩放落到 DOM（其余组件全靠 CSS 变量，不用各管各的）。
 *
 * 字号缩放的做法：所有组件的 `font-size` / `line-height` 都写成
 * `calc(原始值 * var(--xd-font-scale))`，这里只改根上的这一个变量，整界面跟着变，
 * 不用逐个组件通知。
 */
export function applyTheme(): void {
  const root = document.documentElement
  root.dataset.theme = resolveTheme(configStore.cfg.theme_mode)
  root.style.setProperty(
    '--xd-font-scale',
    String(clampUiScale(configStore.cfg.ui_scale) / 100),
  )
  // 卡片不透明度：乘进卡面底色的 alpha（见 style.css 里的 --xd-card 等）
  root.style.setProperty('--xd-glass', String(clampGlass(configStore.cfg.glass_opacity)))
  // 卡片模糊度：卡片背后那层被糊多少（见 style.css 里 .card 的 backdrop-filter）
  const blur = clampCardBlur(configStore.cfg.card_blur)
  root.style.setProperty('--xd-card-blur', `${blur}px`)
  // 门控：0 的时候连 backdrop-filter 都不上 —— 见 style.css 里那条注释（省一层合成层）
  if (blur > 0) root.dataset.glass = '1'
  else delete root.dataset.glass
  // 窗口圆角：`.win` 和两个全屏遮罩都用这个变量，改一处全局跟着变。
  // 窗口是 transparent 的，圆角是**裁**出来的（见 App.vue 的 .win）
  root.style.setProperty(
    '--xd-window-radius',
    `${clampWindowRadius(configStore.cfg.window_radius)}px`,
  )
  // 强调色深浅各存一支，取**当前实际主题**那一支（system 已在上面解析成 light/dark）
  const accent = (
    root.dataset.theme === 'dark' ? configStore.cfg.accent_dark : configStore.cfg.accent_light
  ).trim()
  if (accent.length > 0) {
    root.style.setProperty('--xd-accent', accent)
    root.style.setProperty('--xd-accent-soft', hexToSoft(accent))
    // 压在强调色上的文字**按亮度选黑或白**：荧光绿上写白字等于看不见
    root.style.setProperty('--xd-accent-text', accentTextOn(accent))
  } else {
    root.style.removeProperty('--xd-accent')
    root.style.removeProperty('--xd-accent-soft')
    root.style.removeProperty('--xd-accent-text')
  }
  // 沉浸模式**只在真有壁纸时才生效**：没壁纸时它没有意义，还会把卡片搞成一片雾。
  // 用 data-immersive 而不是行内样式 —— 这条开关要同时改好几个变量和加毛玻璃（都在 CSS 里）。
  const immersive = configStore.cfg.wallpaper_immersive && configStore.cfg.wallpaper_path.length > 0
  if (immersive) root.dataset.immersive = '1'
  else delete root.dataset.immersive
}

/** 强调色的浅底（选中态、hover 底色用） */
function hexToSoft(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim())
  if (m === null) return 'rgba(91, 91, 245, 0.12)'
  const n = parseInt(m[1] ?? '5b5bf5', 16)
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, 0.14)`
}

/** 压在强调色上的文字取黑还是白：亮色底配深字，暗色底配白字 */
function accentTextOn(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim())
  if (m === null) return '#ffffff'
  const n = parseInt(m[1] ?? '000000', 16)
  const lum = (0.299 * ((n >> 16) & 255) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255
  return lum > 0.62 ? '#10120f' : '#ffffff'
}

/** 改配置：整份传回后端（后端有写锁，避免并发覆盖） */
export async function saveConfig(patch: Partial<AppConfig>): Promise<void> {
  const next: AppConfig = { ...configStore.cfg, ...patch }
  configStore.cfg = await tauriApi.configSet(next)
  // 改的分类（domains / domain_labels）要立刻生效，不然得重启才看得见新分类
  syncDomains()
  applyTheme()
}
