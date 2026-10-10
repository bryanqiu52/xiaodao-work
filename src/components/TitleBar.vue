<script setup lang="ts">
// 自制标题栏（窗口无边框，所以标题栏得自己做）。
//
// **窗控一律走后端命令**（`win_*`），不用前端自带的 window API：
// 那条路失败是静默的 —— 桌面窗口里没有 DevTools，表现就是"按钮按了没反应"。
// 走命令之后失败会带回错误文案，这里把它显示成一条红提示，看得见才好修。
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
// 赞赏那颗用点赞，不用钱包：钱包是左侧栏「记账」的图标，两处撞脸会让人以为
// 点顶部这个也是去记账
import { Info, Minus, Pin, PinOff, Square, ThumbsUp, X } from 'lucide-vue-next'
import { isTauri, tauriApi } from '../api/tauri'
import { APP_VERSION } from '../core/constants'
import { configStore } from '../stores/config'

/** 标题栏上的版本号写法跟 tag / Release 一致：小写 v + 三段号 */
const versionText = `v${APP_VERSION}`

const pinned = computed(() => configStore.cfg.always_on_top)

/** 顶部「关于」图标：打开设置并直接停在关于那一页（入口在外壳 App.vue 里接） */
const emit = defineEmits<{ (e: 'open-about'): void }>()

/** 赞赏码浮层开没开 */
const rewardOpen = ref(false)

/**
 * 点关于：先把赞赏码收起来。
 * 两个浮层类的东西叠着开，看上去就是"点了个按钮没反应" —— 那层遮罩还压在下面。
 */
function openAbout(): void {
  rewardOpen.value = false
  emit('open-about')
}

/** Esc 关掉浮层 —— 点了钱包之后想撤，不该只能靠再点一次那个小图标 */
function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') rewardOpen.value = false
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
/**
 * 标题栏透不透壁纸。两个条件都要满足：
 * 有壁纸（没壁纸时透出来只是窗口底色，等于没变） + 设置里打开了「壁纸覆盖顶部」。
 */
const overWallpaper = computed(
  () => configStore.cfg.wallpaper_path.length > 0 && configStore.cfg.wallpaper_header,
)
const busy = ref(false)
const errorHint = ref('')
let hintTimer: number | undefined

/** 窗控出错时给一条看得见的提示，几秒后自己消失 */
function fail(msg: string): void {
  errorHint.value = msg.length > 48 ? `${msg.slice(0, 48)}…` : msg
  if (hintTimer !== undefined) clearTimeout(hintTimer)
  hintTimer = window.setTimeout(() => {
    errorHint.value = ''
  }, 2800)
}

async function run(fn: () => Promise<string | null>): Promise<void> {
  const err = await fn()
  if (err !== null) fail(err)
}

/**
 * 标题栏按下鼠标就开始拖窗口。
 * 按钮和可输入区域要排除掉，否则一点齿轮就变成拖窗口了。
 */
function onDragMouseDown(e: MouseEvent): void {
  if (e.button !== 0) return
  const target = e.target as HTMLElement | null
  if (target !== null && target.closest('button, input, textarea, select, a') !== null) return
  void run(() => tauriApi.winStartDrag())
}

async function togglePin(): Promise<void> {
  if (busy.value) return
  busy.value = true
  const next = !pinned.value
  if (!isTauri()) {
    // 浏览器里调试：改个内存里的状态，让界面还能演示
    configStore.cfg.always_on_top = next
    busy.value = false
    return
  }
  // 后端负责"真的置顶 + 落配置"，成功后再同步界面上的图标状态
  const err = await tauriApi.winSetAlwaysOnTop(next)
  if (err === null) {
    configStore.cfg.always_on_top = next
  } else {
    fail(err)
  }
  busy.value = false
}
</script>

<template>
  <header
    class="tb"
    :class="{ 'tb-over': overWallpaper }"
    data-tauri-drag-region
    @mousedown="onDragMouseDown"
  >
    <div class="tb-left" data-tauri-drag-region>
      <img class="tb-mark" src="../assets/logo.png" alt="XIFOFLY" draggable="false" />
      <span class="tb-title">小刀工作台</span>
      <span class="tb-ver">{{ versionText }}</span>
    </div>

    <div class="tb-right">
      <!-- 赞赏码：置顶左边一个钱包，点了浮出来 -->
      <button
        type="button"
        class="tb-btn"
        :class="{ on: rewardOpen }"
        title="请我喝杯咖啡"
        @click.stop="rewardOpen = !rewardOpen"
      >
        <ThumbsUp :size="14" :stroke-width="2" />
      </button>
      <!-- 关于：直接进设置的「关于」那一页（版本 / 开发者 / 官网） -->
      <button
        type="button"
        class="tb-btn"
        title="关于"
        aria-label="关于"
        @click.stop="openAbout"
      >
        <Info :size="14" :stroke-width="2" />
      </button>
      <button
        type="button"
        class="tb-btn"
        :class="{ on: pinned }"
        :title="pinned ? '取消置顶' : '窗口置顶'"
        @click.stop="togglePin"
      >
        <PinOff v-if="pinned" :size="14" :stroke-width="2" />
        <Pin v-else :size="14" :stroke-width="2" />
      </button>
      <span class="tb-sep" />
      <button
        type="button"
        class="tb-btn win"
        title="最小化"
        @click.stop="run(() => tauriApi.winMinimize())"
      >
        <Minus :size="14" :stroke-width="2.2" />
      </button>
      <button
        type="button"
        class="tb-btn win"
        title="最大化 / 还原"
        @click.stop="run(() => tauriApi.winToggleMaximize())"
      >
        <Square :size="11" :stroke-width="2.2" />
      </button>
      <button
        type="button"
        class="tb-btn win close"
        title="关闭（藏到托盘，托盘图标还在）"
        @click.stop="run(() => tauriApi.winClose())"
      >
        <X :size="14" :stroke-width="2.2" />
      </button>
    </div>

    <div v-if="errorHint" class="tb-hint">{{ errorHint }}</div>
  </header>

  <!-- 赞赏码浮层。Teleport 到 body：标题栏有自己的层叠上下文，
       留在里面会被下面那些面板压住 -->
  <Teleport to="body">
    <!-- 遮罩**从标题栏下沿**开始铺：盖住标题栏的话，
         想再点一次钱包把它收起来就点不到了 -->
    <div v-if="rewardOpen" class="rw-mask" @click="rewardOpen = false"></div>
    <div v-if="rewardOpen" class="rw-pop" role="dialog" aria-label="赞赏码">
      <!-- 走字符串路径，和 logo 一样：Vite 会自己把它当资源处理，不用额外类型声明 -->
      <img class="rw-img" src="../assets/reward.png" alt="溪风设计的赞赏码" draggable="false" />
      <p class="rw-tip">扫一下 · 你的赞赏是对我开发的最大鼓励</p>
    </div>
  </Teleport>
</template>

<style scoped>
.tb {
  position: relative;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 38px;
  /* 左右内边距**跟着窗口圆角走**：圆角大到几十像素时，圆弧会切进标题栏，
     logo 被啃掉一角、窗控按钮被削边（标题栏才 38px 高，半径比它还大）。
     用 max() 让它在小圆角时保持原来的 12 / 8px 不变，圆角一大就自动让开。
     系数是拿几何算的：圆角 r 时，靠近顶边那一点要落在圆弧内，横向至少要让开约 0.5r。 */
  padding: 0 max(8px, calc(var(--xd-window-radius) * 0.55))
    0 max(12px, calc(var(--xd-window-radius) * 0.5));
  background: var(--xd-bg-deep);
  border-bottom: 1px solid var(--xd-border);
  user-select: none;
  /* 整条栏是拖动区，但光标保持默认 —— 抓手图标在这么窄的条上太扎眼，
     且这里是 Windows 桌面应用，拖动窗口本来就该是"默认就能拖"的直觉 */
  cursor: default;
}

/* 窗控失败的提示：贴在标题栏下沿，不占布局 */
.tb-hint {
  position: absolute;
  top: calc(100% - 2px);
  left: 8px;
  right: 8px;
  z-index: 20;
  padding: 5px 9px;
  border-radius: 7px;
  background: var(--xd-card);
  box-shadow: var(--xd-shadow-pop);
  color: var(--xd-red);
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.5;
}

.tb-left {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.tb-mark {
  flex: none;
  width: calc(22px * var(--xd-font-scale));
  height: calc(22px * var(--xd-font-scale));
  border-radius: 6px;
  /* logo 是透明底的黑＋霓虹绿双色画：垫一层白底，深色主题下黑色笔画才看得清 */
  background: #ffffff;
  object-fit: contain;
  box-shadow: 0 0 0 1px rgba(16, 24, 40, 0.08);
}

.tb-title {
  font-size: calc(15px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

/* 版本号：字号比标题小一档、颜色也压暗一档 —— 它在标题栏里是"标识"不是"内容"，
   跟标题一样抢眼就成噪音了 */
.tb-ver {
  flex: none;
  font-size: calc(11px * var(--xd-font-scale));
  font-weight: 500;
  color: var(--xd-text-dim);
}

/* 壁纸覆盖顶部：标题栏让位，只留一点磨砂。
   完全透明不行 —— 壁纸一深一浅，标题字会像浮在半空没有依托；垫一层薄磨砂就有底了。 */
.tb.tb-over {
  background: color-mix(in srgb, var(--xd-bg-deep) 30%, transparent);
  -webkit-backdrop-filter: blur(14px) saturate(150%);
  backdrop-filter: blur(14px) saturate(150%);
}

.tb-right {
  display: flex;
  align-items: center;
  gap: 1px;
}

.tb-sep {
  width: 1px;
  height: 14px;
  margin: 0 3px;
  background: var(--xd-border);
}

.tb-btn {
  display: grid;
  place-items: center;
  width: calc(28px * var(--xd-font-scale));
  height: calc(26px * var(--xd-font-scale));
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--xd-text-dim);
  transition: all 0.14s var(--xd-ease);
}

.tb-btn:hover {
  background: var(--xd-border-soft);
  color: var(--xd-text);
}

.tb-btn.on {
  color: var(--xd-accent);
  background: var(--xd-accent-soft);
}

.tb-btn.close:hover {
  background: color-mix(in srgb, var(--xd-red) 16%, transparent);
  color: var(--xd-red);
}

/* ── 赞赏码浮层 ────────────────────────────────────────────── */
.rw-mask {
  position: fixed;
  left: 0;
  right: 0;
  /* 从标题栏下沿开始，标题栏留着可点 —— 才能再点一次钱包收起来 */
  top: 38px;
  bottom: 0;
  z-index: 80;
  /* 遮罩铺满视口，而窗口四角是透明的，这里跟着切一刀，不然会糊在窗外那四个角上 */
  border-radius: 0 0 var(--xd-window-radius) var(--xd-window-radius);
}

.rw-pop {
  position: fixed;
  top: 42px;
  right: 10px;
  z-index: 85;
  padding: 8px 8px 6px;
  border: 1px solid var(--xd-border);
  border-radius: 14px;
  background: var(--xd-card);
  box-shadow: var(--xd-shadow-pop);
  animation: rw-in 0.16s var(--xd-ease);
}

@keyframes rw-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.rw-img {
  display: block;
  width: min(290px, calc(100vw - 40px));
  border-radius: 9px;
  /* 二维码类图片**必须留白底**：反色就扫不出来了，深色主题也不例外 */
  background: #ffffff;
}

.rw-tip {
  margin: 6px 0 1px;
  text-align: center;
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}
</style>
