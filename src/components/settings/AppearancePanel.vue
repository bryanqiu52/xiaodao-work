<script setup lang="ts">
// 外观：主题模式 + 强调色。
// 改完立刻生效（applyTheme 把值写进 CSS 变量，所有组件跟着变，不用刷新）。
import { computed, inject, onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Check } from 'lucide-vue-next'
import {
  clampCardBlur,
  clampGlass,
  clampUiScale,
  clampVeil,
  clampWindowRadius,
  RADIUS_DEFAULT,
  RADIUS_MAX,
  RADIUS_MIN,
  RADIUS_STEP,
  VEIL_MAX,
} from '../../core/config'
import { isTauri, logToBackend, tauriApi, type BuiltinWallpaper } from '../../api/tauri'
import { applyTheme, configStore, saveConfig } from '../../stores/config'

const showToast = inject<(s: string) => void>('showToast', () => {})

/**
 * 字号缩放：拖动时**只改内存里的值并立即应用**（拖动是连续的，不能每动一下就写盘），
 * 松手（change）才真正落配置。
 */
const uiScale = computed(() => clampUiScale(configStore.cfg.ui_scale))

function previewScale(e: Event): void {
  configStore.cfg.ui_scale = clampUiScale(Number((e.target as HTMLInputElement).value))
  applyTheme()
}

function commitScale(): void {
  void saveConfig({ ui_scale: clampUiScale(configStore.cfg.ui_scale) })
}

function resetScale(): void {
  configStore.cfg.ui_scale = 100
  applyTheme()
  void saveConfig({ ui_scale: 100 })
  showToast('字号已恢复 100%')
}

const THEMES = [
  { v: 'light', label: '浅色' },
  { v: 'dark', label: '深色' },
  { v: 'system', label: '跟随系统' },
]

/**
 * 色板。头两个是**亮色 / 暗色各自的默认值**（原插件的口径：亮色蓝、暗色荧光绿），
 * 放在最前面，一眼能找到"回去"的那两个。
 */
const SWATCHES = ['#4176e6', '#c8ff3e', '#2e9e6b', '#d9822b', '#d64545', '#7c5cff']

async function setTheme(v: string): Promise<void> {
  await saveConfig({ theme_mode: v })
}

/**
 * 主题内置那两支强调色：亮蓝 / 暗绿。
 * 没设过就用对应这支 —— 也是色板打勾和取色器的初始值。
 */
const ACCENT_DEFAULT = { light: '#4176e6', dark: '#c8ff3e' }

/**
 * 设某一支强调色。**深浅分开存**：一支色伺候不了两个底色 ——
 * 暗色下好看的荧光绿放到白底上会糊成一片，亮蓝压在深底上又发闷。
 */
async function setAccent(which: 'light' | 'dark', hex: string): Promise<void> {
  await saveConfig(which === 'dark' ? { accent_dark: hex } : { accent_light: hex })
  showToast(`${which === 'dark' ? '深色' : '浅色'}模式的强调色已换成 ${hex}`)
}

/** 恢复默认：把那支清空即可 —— 空串的含义就是"用内置这支" */
async function resetAccent(which: 'light' | 'dark'): Promise<void> {
  await saveConfig(which === 'dark' ? { accent_dark: '' } : { accent_light: '' })
  showToast(`${which === 'dark' ? '深色' : '浅色'}模式的强调色已恢复默认`)
}

async function customAccent(which: 'light' | 'dark', e: Event): Promise<void> {
  await setAccent(which, (e.target as HTMLInputElement).value)
}

/** 某一支实际在用的颜色：没设过就是内置这支 */
function shownAccent(which: 'light' | 'dark'): string {
  const raw = which === 'dark' ? configStore.cfg.accent_dark : configStore.cfg.accent_light
  return raw.trim().length > 0 ? raw.trim() : ACCENT_DEFAULT[which]
}

// ── 背景：卡片不透明度 + 壁纸 ────────────────────────────────────────────

/**
 * 卡片不透明度。滑块和字号那档一个套路：**拖动只改内存并立即生效，松手才落盘**
 * —— 拖动是连续的，每动一下就写盘既没必要也伤盘。
 */
const glassPercent = computed(() => Math.round(clampGlass(configStore.cfg.glass_opacity) * 100))

function previewGlass(e: Event): void {
  configStore.cfg.glass_opacity = clampGlass(Number((e.target as HTMLInputElement).value) / 100)
  applyTheme()
}

function commitGlass(): void {
  void saveConfig({ glass_opacity: clampGlass(configStore.cfg.glass_opacity) })
}

function resetGlass(): void {
  configStore.cfg.glass_opacity = 1
  applyTheme()
  void saveConfig({ glass_opacity: 1 })
  showToast('卡片不透明度已恢复 100%')
}

/**
 * 卡片模糊度：卡片背后的东西被糊多少。
 * 跟不透明度同一套路（拖动只改内存并立即生效，松手才落盘）。
 * 默认 0 = 不糊 —— 给每张卡片都开一层模糊是实打实的合成开销，想要的人自己开。
 */
const cardBlur = computed(() => clampCardBlur(configStore.cfg.card_blur))

function previewCardBlur(e: Event): void {
  configStore.cfg.card_blur = clampCardBlur(Number((e.target as HTMLInputElement).value))
  applyTheme()
}

function commitCardBlur(): void {
  void saveConfig({ card_blur: clampCardBlur(configStore.cfg.card_blur) })
}

/**
 * 窗口圆角。跟卡片不透明度同一套路：拖动时只改内存里的值并立即应用到 DOM
 * （拖动是连续的，不能每动一下就写盘），松手才落配置。
 */
const radiusValue = computed(() => clampWindowRadius(configStore.cfg.window_radius))

function previewRadius(e: Event): void {
  configStore.cfg.window_radius = clampWindowRadius(Number((e.target as HTMLInputElement).value))
  applyTheme()
}

function commitRadius(): void {
  void saveConfig({ window_radius: clampWindowRadius(configStore.cfg.window_radius) })
}

function resetRadius(): void {
  configStore.cfg.window_radius = RADIUS_DEFAULT
  applyTheme()
  void saveConfig({ window_radius: RADIUS_DEFAULT })
  showToast('窗口圆角已恢复默认')
}

/**
 * 有没有壁纸。
 * 没有的话，下面三行（蒙版 / 背景处理 / 覆盖顶部）全是空转 ——
 * 它们改的都是"壁纸怎么显示"，没壁纸时调了也看不出任何变化。
 * 那种"能点但没反应"最容易被当成坏掉，所以直接置灰并禁用。
 */
const hasWallpaper = computed(() => configStore.cfg.wallpaper_path.length > 0)

/** 只显示壁纸文件名：完整路径太长，这一行放不下 */
const wallpaperName = computed(() => {
  const p = configStore.cfg.wallpaper_path
  if (p.length === 0) return ''
  const parts = p.split(/[\\/]/)
  return parts[parts.length - 1] ?? p
})

async function pickWallpaper(): Promise<void> {
  if (!isTauri()) {
    showToast('浏览器里选不了壁纸')
    return
  }
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif'] }],
    })
    if (typeof picked !== 'string') return
    const stored = await tauriApi.importWallpaper(picked)
    if (stored.startsWith('error:')) {
      // 导入失败必须说出来：不说的话用户只看到"选了图但没反应"
      logToBackend('error', `壁纸导入失败：${stored}`)
      showToast(`壁纸导入失败：${stored.slice(6)}`)
      return
    }
    await saveConfig({ wallpaper_path: stored })
    showToast('壁纸已更新')
  } catch (e) {
    logToBackend('error', `选壁纸失败：${String(e)}`)
    showToast(`选图失败：${String(e)}`)
  }
}

/**
 * 移除壁纸：**只删当前这一张**。
 *
 * 顺序是"先记住路径 → 清配置 → 再删文件"：先清配置就读不到该删哪张了；
 * 而按这个顺序，就算删失败也只是留下一张没人用的图，不会变成"配置指着已删的图"。
 * 以前这里是清空整个壁纸目录，用户自己放进去的图跟着一起没了（2026-09-25 修）。
 */
async function clearWallpaper(): Promise<void> {
  const current = configStore.cfg.wallpaper_path
  await saveConfig({ wallpaper_path: '' })
  if (current.length === 0) return
  const err = await tauriApi.removeWallpaper(current)
  // 删不掉也要说：壁纸已经取消，但盘上那张还在 —— 不说就成了"点了没反应"
  showToast(err === null ? '已移除壁纸' : `壁纸已取消，但图没删掉：${err}`)
}

// ── 内置壁纸：随安装包分发的 `Wallpaper` 文件夹 ───────────────────────────

/**
 * 内置壁纸清单。**只在面板挂载时读一次** —— 图是随程序带来的，运行中不增不减，
 * 切个大类就重新问一遍纯属浪费。
 */
const builtins = ref<BuiltinWallpaper[]>([])
/** 一张都没有时的说明。说清图该放哪儿，比留一片空白强 */
const builtinEmpty = ref('')

onMounted(async () => {
  builtins.value = await tauriApi.builtinWallpapers()
  if (builtins.value.length === 0) {
    // 内置壁纸是随程序带来的，不做"自己往文件夹里放图"的引导 ——
    // 那会让人以为要去动安装目录。真没有就直说，想要别的图走上面的「选择图片…」
    builtinEmpty.value = isTauri()
      ? '这一版没带内置壁纸。想要别的背景，用上面的「选择图片…」挑一张本地图'
      : '浏览器调试模式读不到内置壁纸（没有资源目录）'
  }
})

/** 缩略图地址：本地图片得经 asset 协议 WebView 才读得到 */
function thumb(w: BuiltinWallpaper): string {
  return isTauri() ? convertFileSrc(w.path) : ''
}

/** 当前壁纸的文件名（去扩展名） */
const currentStem = computed(() => {
  const p = configStore.cfg.wallpaper_path
  if (p.length === 0) return ''
  const parts = p.split(/[\\/]/)
  const file = parts[parts.length - 1] ?? ''
  const dot = file.lastIndexOf('.')
  return dot > 0 ? file.slice(0, dot) : file
})

/**
 * 选中态**靠文件名认**：内置图拷进数据根时固定叫 `内置-<图名>`（见后端 import_wallpaper）。
 * 自选的图文件名带时间戳，认不出来，所以不会把自选的误判成内置。
 */
function isActive(w: BuiltinWallpaper): boolean {
  return currentStem.value === `内置-${w.name}`
}

async function pickBuiltin(w: BuiltinWallpaper): Promise<void> {
  // 内置图**同样拷进数据根**：之后备份 / 恢复 / 清空统统只认数据根里那一份，
  // 不用为"图库"再开一套逻辑 —— 装目录里的那张永远只是"原图"
  const stored = await tauriApi.importWallpaper(w.path, w.name)
  if (stored.startsWith('error:')) {
    // 导入失败必须说出来：不说的话用户只看到"点了没反应"
    logToBackend('error', `内置壁纸导入失败：${stored}`)
    showToast(`换不了：${stored.slice(6)}`)
    return
  }
  await saveConfig({ wallpaper_path: stored })
  showToast(`已换成「${w.name}」`)
}

/** 壁纸蒙版浓度 */
const veilPercent = computed(() => Math.round(clampVeil(configStore.cfg.wallpaper_veil) * 100))

function previewVeil(e: Event): void {
  configStore.cfg.wallpaper_veil = clampVeil(Number((e.target as HTMLInputElement).value) / 100)
}

function commitVeil(): void {
  void saveConfig({ wallpaper_veil: clampVeil(configStore.cfg.wallpaper_veil) })
}

/**
 * 背景处理：**三选一，互斥**。
 *
 * 原来「背景模糊」和「沉浸模式」是两个独立开关，但同时打开时模糊是**静默失效**的
 * （见 App.vue 的 wallpaperBlur）—— 用户看到两个都亮着却只有一个生效，只会以为坏了。
 * 改成单选就没这个歧义了。
 */
const WALLPAPER_MODES = [
  { value: 'none', label: '不处理' },
  { value: 'blur', label: '整张模糊' },
  { value: 'immersive', label: '沉浸模式' },
] as const
type WallpaperMode = (typeof WALLPAPER_MODES)[number]['value']

/**
 * 当前档位从两个布尔字段反推 —— 不新增字段，老配置照样读得出来；
 * 两个都为真（历史遗留的歧义状态）时按"沉浸优先"，与渲染时的口径一致。
 */
const wallpaperMode = computed<WallpaperMode>(() => {
  if (configStore.cfg.wallpaper_immersive) return 'immersive'
  if (configStore.cfg.wallpaper_blur) return 'blur'
  return 'none'
})

async function setWallpaperMode(m: WallpaperMode): Promise<void> {
  // 两个字段一起写死，不留"两个都为真"的歧义
  await saveConfig({
    wallpaper_immersive: m === 'immersive',
    wallpaper_blur: m === 'blur',
  })
  if (m === 'immersive' && configStore.cfg.wallpaper_path.length === 0) {
    showToast('沉浸模式要配合壁纸才看得出效果')
  }
}

async function toggleHeader(): Promise<void> {
  await saveConfig({ wallpaper_header: !configStore.cfg.wallpaper_header })
}
</script>

<template>
  <!-- 主题 -->
  <section id="sv-sec-theme" class="sv-sec">
    <h3 class="sv-sec-title">主题</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">主题模式</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="t in THEMES"
          :key="t.v"
          type="button"
          :class="{ on: configStore.cfg.theme_mode === t.v }"
          @click="setTheme(t.v)"
        >
          {{ t.label }}
        </button>
      </div>
    </div>

  </section>

  <!-- 字体 -->
  <section id="sv-sec-font" class="sv-sec">
    <h3 class="sv-sec-title">字体</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">界面字体</span>
        <span class="setting-desc">缩放字号与行高，间距不动；调到 150% 以上建议同时拉宽窗口</span>
      </div>
      <div class="sv-scale-box">
        <input
          class="sv-range"
          type="range"
          min="85"
          max="200"
          step="5"
          :value="uiScale"
          aria-label="界面字体百分比"
          @input="previewScale"
          @change="commitScale"
        />
        <span class="sv-scale-n">{{ uiScale }}%</span>
        <button type="button" class="sv-btn" @click="resetScale">重置</button>
      </div>
    </div>

  </section>

  <!-- 强调色：深浅各一支 -->
  <section id="sv-sec-accent" class="sv-sec">
    <h3 class="sv-sec-title">强调色</h3>
    <p class="sv-sec-note">按钮、选中态、编号角标的颜色。深浅两套各存一支</p>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">浅色模式</span>
        <span class="setting-desc">默认 {{ ACCENT_DEFAULT.light }}，当前 {{ shownAccent('light') }}</span>
      </div>
      <div class="sv-swatches">
        <button
          v-for="c in SWATCHES"
          :key="c"
          type="button"
          class="sv-swatch"
          :class="{ on: shownAccent('light').toLowerCase() === c }"
          :style="{ background: c }"
          :title="c"
          @click="setAccent('light', c)"
        />
        <input
          class="sv-swatch"
          style="padding: 0; border-width: 2px; cursor: pointer"
          type="color"
          :value="shownAccent('light')"
          title="自定义"
          @change="customAccent('light', $event)"
        />
        <button type="button" class="sv-btn" @click="resetAccent('light')">恢复默认</button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">深色模式</span>
        <span class="setting-desc">默认 {{ ACCENT_DEFAULT.dark }}，当前 {{ shownAccent('dark') }}</span>
      </div>
      <div class="sv-swatches">
        <button
          v-for="c in SWATCHES"
          :key="c"
          type="button"
          class="sv-swatch"
          :class="{ on: shownAccent('dark').toLowerCase() === c }"
          :style="{ background: c }"
          :title="c"
          @click="setAccent('dark', c)"
        />
        <input
          class="sv-swatch"
          style="padding: 0; border-width: 2px; cursor: pointer"
          type="color"
          :value="shownAccent('dark')"
          title="自定义"
          @change="customAccent('dark', $event)"
        />
        <button type="button" class="sv-btn" @click="resetAccent('dark')">恢复默认</button>
      </div>
    </div>
  </section>

  <!-- 背景：卡片不透明度 + 壁纸 -->
  <section id="sv-sec-background" class="sv-sec">
    <h3 class="sv-sec-title">背景</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">卡片不透明度</span>
        <span class="setting-desc">调低会透出背景和壁纸；低于 60% 字会受干扰</span>
      </div>
      <div class="sv-scale-box">
        <input
          class="sv-range"
          type="range"
          min="40"
          max="100"
          step="1"
          :value="glassPercent"
          aria-label="卡片不透明度百分比"
          @input="previewGlass"
          @change="commitGlass"
        />
        <span class="sv-scale-n">{{ glassPercent }}%</span>
        <button type="button" class="sv-btn" @click="resetGlass">重置</button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">卡片模糊度</span>
        <span class="setting-desc">
          把卡片背后的东西糊掉多少。默认 0（不糊）；配合上面「卡片不透明度」调低才看得出来 ——
          沉浸模式下至少 16px
        </span>
      </div>
      <div class="sv-scale-box">
        <input
          class="sv-range"
          type="range"
          min="0"
          max="40"
          step="2"
          :value="cardBlur"
          aria-label="卡片模糊度"
          @input="previewCardBlur"
          @change="commitCardBlur"
        />
        <span class="sv-scale-n">{{ cardBlur }}px</span>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">窗口圆角</span>
        <span class="setting-desc">0 是直角，16 接近 iOS 卡片，50 差不多是手机屏幕那种圆</span>
      </div>
      <div class="sv-scale-box">
        <input
          class="sv-range"
          type="range"
          :min="RADIUS_MIN"
          :max="RADIUS_MAX"
          :step="RADIUS_STEP"
          :value="radiusValue"
          aria-label="窗口圆角像素"
          @input="previewRadius"
          @change="commitRadius"
        />
        <span class="sv-scale-n">{{ radiusValue }}px</span>
        <button type="button" class="sv-btn" @click="resetRadius">重置</button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">壁纸</span>
        <span class="setting-desc">选一张图当背景。图存进数据目录，原图挪走也不影响</span>
      </div>
      <div class="sv-swatches">
        <span v-if="wallpaperName" class="sv-file-name" :title="configStore.cfg.wallpaper_path">
          {{ wallpaperName }}
        </span>
        <button type="button" class="sv-btn" @click="pickWallpaper">
          {{ wallpaperName ? '换一张…' : '选择图片…' }}
        </button>
        <button v-if="wallpaperName" type="button" class="sv-btn" @click="clearWallpaper">
          移除
        </button>
      </div>
    </div>

    <!-- 内置壁纸：随安装包带来的图，挑一张即用（同样拷进数据目录，后续逻辑完全一致） -->
    <div class="setting-row sub">
      <div class="setting-info">
        <span class="setting-name">内置壁纸</span>
        <span class="setting-desc">随程序一起装上的图，点一下就用</span>
      </div>
      <div v-if="builtins.length > 0" class="wp-grid">
        <button
          v-for="w in builtins"
          :key="w.path"
          type="button"
          class="wp-card"
          :class="{ on: isActive(w) }"
          :title="w.name"
          @click="pickBuiltin(w)"
        >
          <img class="wp-thumb" :src="thumb(w)" :alt="w.name" draggable="false" />
          <span class="wp-name">{{ w.name }}</span>
          <span v-if="isActive(w)" class="wp-check" aria-hidden="true">
            <Check :size="10" :stroke-width="3" />
          </span>
        </button>
      </div>
      <p v-else class="wp-empty">{{ builtinEmpty }}</p>
    </div>

    <div class="setting-row sub" :class="{ disabled: !hasWallpaper }">
      <div class="setting-info">
        <span class="setting-name">壁纸蒙版</span>
        <span class="setting-desc">压暗壁纸，让卡片上的字更清楚</span>
      </div>
      <div class="sv-scale-box">
        <input
          class="sv-range"
          type="range"
          min="0"
          :max="Math.round(VEIL_MAX * 100)"
          step="1"
          :value="veilPercent"
          :disabled="!hasWallpaper"
          aria-label="壁纸蒙版百分比"
          @input="previewVeil"
          @change="commitVeil"
        />
        <span class="sv-scale-n">{{ veilPercent }}%</span>
      </div>
    </div>

    <div class="setting-row sub" :class="{ disabled: !hasWallpaper }">
      <div class="setting-info">
        <span class="setting-name">背景处理</span>
        <span class="setting-desc">整张模糊 = 虚化壁纸；沉浸 = 卡片变毛玻璃、透出壁纸轮廓</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="m in WALLPAPER_MODES"
          :key="m.value"
          type="button"
          class="sv-btn"
          :class="{ on: wallpaperMode === m.value }"
          :disabled="!hasWallpaper"
          @click="setWallpaperMode(m.value)"
        >
          {{ m.label }}
        </button>
      </div>
    </div>

    <div class="setting-row sub" :class="{ disabled: !hasWallpaper }">
      <div class="setting-info">
        <span class="setting-name">壁纸覆盖顶部</span>
        <span class="setting-desc">顶部标题栏也透出壁纸（垫了一层薄磨砂保证标题看得清）</span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.wallpaper_header"
        :class="{ on: configStore.cfg.wallpaper_header }"
        :disabled="!hasWallpaper"
        @click="toggleHeader"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.sv-scale-box {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 竖排之后一整行都是它的，不必再挤在 104px 里 —— 长一点更好拖 */
.sv-range {
  width: 170px;
  accent-color: var(--xd-accent);
  cursor: pointer;
}

/* 固定宽度：数字从 100% 跳到 85% 时不会把右边的按钮推着动 */
.sv-scale-n {
  width: 38px;
  text-align: right;
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(13.8px * var(--xd-font-scale));
  color: var(--xd-text-sub);
}

/* ── 内置壁纸卡片 ────────────────────────────────────────────────────── */
/* 卡片区要占满整行：.setting-row 里非说明的部分默认是 align-self: flex-start，
   网格不撑开的话会缩到内容宽度、两三张挤成一团 */
.wp-grid {
  align-self: stretch;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
  gap: 8px;
  width: 100%;
  margin-top: 2px;
}

.wp-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--xd-border-soft);
  border-radius: 10px;
  background: var(--xd-card-sub);
  transition: all 0.16s var(--xd-ease);
}

.wp-card:hover {
  transform: translateY(-2px);
  border-color: var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
}

/* 选中态：强调色描边 + 角上一个勾。图本身千差万别，只能靠边框说话 */
.wp-card.on {
  border-color: var(--xd-accent);
}

.wp-thumb {
  display: block;
  width: 100%;
  aspect-ratio: 1 / 1;
  object-fit: cover;
  border-radius: 7px;
  background: var(--xd-border-soft);
}

.wp-name {
  font-size: calc(12px * var(--xd-font-scale));
  line-height: 1.4;
  color: var(--xd-text-sub);
  text-align: center;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.wp-card.on .wp-name {
  color: var(--xd-accent);
  font-weight: 600;
}

.wp-check {
  position: absolute;
  top: 7px;
  right: 7px;
  display: grid;
  place-items: center;
  width: calc(16px * var(--xd-font-scale));
  height: calc(16px * var(--xd-font-scale));
  border-radius: 50%;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
}

.wp-empty {
  margin: 2px 0 0;
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.55;
  color: var(--xd-text-dim);
}

/* 壁纸文件名：只占一行，太长就省略（完整路径在 title 里） */
.sv-file-name {
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text-sub);
}
</style>
