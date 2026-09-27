<script setup lang="ts">
// 应用外壳：标题栏 + 待办面板 / 设置页 二选一。
//
// 关窗口默认是**藏到托盘**而不是退出（标题栏的关闭按钮走 hide），
// 托盘常驻才是这个东西的用法：想到就按快捷键叫出来，不用去任务栏找。
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { convertFileSrc } from '@tauri-apps/api/core'
import { isTauri, logToBackend, tauriApi } from './api/tauri'
import { clampVeil } from './core/config'
import TitleBar from './components/TitleBar.vue'
import SideRail, { type MainView } from './components/SideRail.vue'
import TodoPane from './components/TodoPane.vue'
import FocusPane from './components/FocusPane.vue'
import GithubPane from './components/GithubPane.vue'
import MoneyPane from './components/MoneyPane.vue'
import ReviewPane from './components/ReviewPane.vue'
import SettingsView from './components/settings/SettingsView.vue'
import WelcomeWizard from './components/WelcomeWizard.vue'
import ConfirmSheet from './components/ConfirmSheet.vue'
import UpdateSheet from './components/UpdateSheet.vue'
import { loadConfig, configStore, applyTheme, saveConfig } from './stores/config'
import { loadTodos } from './stores/todo'
import { useReminder } from './composables/useReminder'
import { startFocus, stopFocus } from './composables/useFocus'
import { loadFocusLog } from './composables/useFocusLog'
import { checkUpdate } from './composables/useUpdate'

// 到期提醒：订阅 Rust 的定时 tick，该提醒时发系统通知
useReminder()

const view = ref<MainView>('todo')
/** 进设置前停在哪个主视图 —— 关设置回那里，而不是粗暴弹回待办 */
const lastMain = ref<Exclude<MainView, 'settings'>>('todo')

/** 打开设置时停在哪个大类。只有「常规」和「关于」两种入口，够用就不铺开 */
type SettingsGroup = 'general' | 'about'
const settingsGroup = ref<SettingsGroup>('general')

function navigate(v: MainView): void {
  if (v !== 'settings') lastMain.value = v
  // 从侧边栏 / 托盘进设置 = 从常规那一页看起
  else settingsGroup.value = 'general'
  view.value = v
}

/** 顶部标题栏那个「关于」图标：打开设置并直接停在关于 */
function openAbout(): void {
  settingsGroup.value = 'about'
  view.value = 'settings'
}

function closeSettings(): void {
  view.value = lastMain.value
}

/** 首次启动向导：没看过就弹一次 */
const welcomeOpen = ref(false)

async function finishWelcome(): Promise<void> {
  welcomeOpen.value = false
  // 记进配置：看过就不再弹。引导这种东西第二次出现就是打扰，
  // 所以「跳过」和「开始使用」一样 —— 都算看过了
  await saveConfig({ welcome_done: true })
}

/**
 * 壁纸。本地图片得经 Tauri 的 asset 协议才能被 WebView 读到，
 * 所以路径要先过 `convertFileSrc`（它把绝对路径转成 `asset://` 那种可访问的 URL）。
 */
const wallpaperSrc = computed(() => {
  const p = configStore.cfg.wallpaper_path
  if (p.length === 0) return ''
  return isTauri() ? convertFileSrc(p) : ''
})

/** 蒙版浓度：夹回安全区间（配置被手改坏也不至于把壁纸压成一片纯色） */
const wallpaperVeil = computed(() => clampVeil(configStore.cfg.wallpaper_veil))

/**
 * 要不要给壁纸加模糊。
 * 沉浸模式下**关掉**：糊活已经交给卡片的毛玻璃了，两层糊叠起来画面会发肉。
 */
const wallpaperBlur = computed(
  () => configStore.cfg.wallpaper_blur && !configStore.cfg.wallpaper_immersive,
)

/** 桌面通道没连上时的醒目提示。空串 = 一切正常 */
const envNote = ref('')

// 贴边隐藏：鼠标蹭到屏幕边上那条缝就把窗口滑出来，移开一会儿再收回。
// 收起不打招呼也不留 UI 痕迹，所以这里必须幂等 —— 判定（是不是真贴着边、
// 是不是真处于收起状态）全交给 Rust，前端只管把动静传过去。
const EDGE_LEAVE_DELAY = 700
let leaveTimer: number | undefined

function cancelEdgeHide(): void {
  if (leaveTimer !== undefined) {
    clearTimeout(leaveTimer)
    leaveTimer = undefined
  }
}

function onWindowEnter(): void {
  cancelEdgeHide()
  void tauriApi.edgeReveal()
}

function onWindowLeave(): void {
  cancelEdgeHide()
  leaveTimer = window.setTimeout(() => {
    leaveTimer = undefined
    void tauriApi.edgeHide()
  }, EDGE_LEAVE_DELAY)
}

onMounted(async () => {
  await loadConfig()
  await loadTodos()
  // 专注记录：番茄页（"这条累计几个番茄"）和回顾页都要它。
  // 在 App 层读一次就够 —— 两个页面共用同一份内存，避免各读各的、数字还对不上
  void loadFocusLog()
  // 番茄钟的后台计时。在 App 层启动（不是番茄钟界面里）——
  // 界面切走时组件会被卸载，挂在组件里的计时就跟着停了。
  // 状态是模块级单例，这里只管把节拍器拧上。
  startFocus()

  // 首次启动向导：没看过就弹一次（配置里记着看没看过）
  if (!configStore.cfg.welcome_done) welcomeOpen.value = true

  // 自检：桌面通道到底连上没有。
  // 连不上时一切改动都只停在内存里（"点了没反应"的老毛病就是这么来的），
  // 与其假装能用，不如直接把提示顶在界面上，一眼能看出来。
  const probe = await tauriApi.envProbe()
  logToBackend('info', `启动自检 ${probe}`)
  if (!probe.startsWith('version=')) {
    envNote.value = `没连上桌面接口（${probe}）—— 现在的改动不会保存`
  } else {
    // 快捷键注册失败原来只在日志里说一句，用户只会发现"快捷键没反应"却查不出原因
    // —— 这种事必须摆到脸上。桌面通道是通的才轮到它说话（上面那条更要紧）。
    const st = await tauriApi.shortcutState()
    logToBackend('info', `快捷键状态 ${st}`)
    if (st !== 'ok') {
      envNote.value =
        st === 'conflict'
          ? '全局快捷键被别的程序占用了，去「设置 → 常规」换一个'
          : `全局快捷键没生效（${st}），去「设置 → 常规」看看`
    }
  }

  // 查更新：**等一会儿再查**。启动这一瞬间有一堆活在排队（配置、清单、专注记录、
  // 快捷键注册……），没必要再挤一个网络请求进去；晚几秒用户已经看到界面了，
  // 真弹出提示也不显得突兀。**一天最多一次**，节流在 `checkUpdate` 里。
  window.setTimeout(() => void checkUpdate(false), 8000)

  // 主题跟随系统时，系统切了这边要跟着变
  if (typeof window !== 'undefined' && window.matchMedia) {
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    mq.addEventListener('change', () => {
      if (configStore.cfg.theme_mode === 'system') applyTheme()
    })
  }
  // 托盘菜单里的「设置」
  if (isTauri()) {
    // 托盘那一下点没点着、事件有没有到，全靠日志分：Rust 侧记「派发了」，
    // 这里记「收到了」。两头都记，才不用猜是发丢了还是收漏了。
    try {
      await listen('open-settings', () => {
        logToBackend('info', '收到 open-settings，切到设置页')
        navigate('settings')
      })
    } catch (e) {
      logToBackend('error', `监听 open-settings 失败：${String(e)}`)
    }
    document.addEventListener('mouseenter', onWindowEnter)
    document.addEventListener('mouseleave', onWindowLeave)
  }
})

onBeforeUnmount(() => {
  cancelEdgeHide()
  stopFocus()
  document.removeEventListener('mouseenter', onWindowEnter)
  document.removeEventListener('mouseleave', onWindowLeave)
})

// 影响主题的几个值变了就重新应用（设置页里改了立刻见效）。
// 走 saveConfig 本来就会调 applyTheme，这条是兜底：浏览器调试时直接改内存也要跟得上。
watch(
  () => [
    configStore.cfg.theme_mode,
    configStore.cfg.glass_opacity,
    configStore.cfg.card_blur,
    configStore.cfg.wallpaper_path,
    configStore.cfg.wallpaper_immersive,
  ],
  () => applyTheme(),
)
</script>

<template>
  <!-- `.win` 是窗口的"脸"：圆角 + 底色 + 裁剪都归它管。
       **它必须包住壁纸层** —— 壁纸是定位元素，body 上写 border-radius 裁不到它，
       圆角就会在壁纸上露出直角。 -->
  <div class="win">
    <!-- 窗口底色单独一层，**放在壁纸下面**。
         以前底色写在 `.win` 自己身上，结果把壁纸压没了 ——
         父元素的背景虽然画在正常流内容之下，却画在定位子元素的**负层之上**，
         靠 z-index:-1 去垫底是在赌绘制顺序。改成老老实实按 DOM 顺序叠：
         底色 → 壁纸 → 内容，一眼看得出谁在谁上面。 -->
    <div class="win-bg" aria-hidden="true" />

    <!-- 壁纸：垫在底色之上、所有内容之下。
         卡片想透出它就把「卡片不透明度」调低；开了沉浸模式则是毛玻璃透出它的轮廓。 -->
    <div v-if="wallpaperSrc" class="xd-wallpaper" aria-hidden="true">
      <img class="xd-wallpaper-img" :class="{ blur: wallpaperBlur }" :src="wallpaperSrc" alt="" />
      <div class="xd-wallpaper-veil" :style="{ opacity: String(wallpaperVeil) }" />
    </div>

    <div class="shell">
      <SideRail :view="view" @navigate="navigate" />
      <div class="shell-main">
        <!-- 设置入口在侧边栏底部，标题栏不再重复放一个齿轮 -->
        <TitleBar @open-about="openAbout" />
        <p v-if="envNote !== ''" class="env-note">{{ envNote }}</p>
        <main class="shell-body">
          <TodoPane v-if="view === 'todo'" />
          <FocusPane v-else-if="view === 'focus'" />
          <GithubPane v-else-if="view === 'github'" />
          <MoneyPane v-else-if="view === 'money'" />
          <ReviewPane v-else-if="view === 'review'" />
          <SettingsView v-else :initial-group="settingsGroup" @close="closeSettings" />
        </main>
      </div>
    </div>

    <!-- 首次启动向导。它自己 Teleport 到 body，所以挂在最外层就行 -->
    <WelcomeWizard v-if="welcomeOpen" @close="finishWelcome" />

    <!-- 通用确认浮层：**常驻挂一个就够**，全应用共用一个实例（见 useConfirm.ts）。
         它自己 Teleport 到 body，放哪儿都行；但必须挂在最外层 ——
         设置页、记账页都要能唤起它，挂进任何一个页面里都会被那个页面卸载带走 -->
    <ConfirmSheet />

    <!-- 发现新版本的浮层。也是常驻，有新版时自己弹（见 useUpdate.ts） -->
    <UpdateSheet />
  </div>
</template>

<style scoped>
/**
 * 窗口圆角靠这一层裁出来。
 *
 * 窗口本体是 `transparent` 的（见 tauri.conf.json），四角空出来的地方直接透出桌面，
 * 这里的 `border-radius` + `overflow: hidden` 才是有形状的那一刀。
 * **包住壁纸层是必须的**：壁纸用绝对定位，body 上的圆角裁不到它 ——
 * 只在 body 上写圆角的话，卡片圆了、壁纸还是直角。
 */
.win {
  position: relative;
  height: 100%;
  border-radius: var(--xd-window-radius);
  overflow: hidden;
  /* **这里绝对不能写 background**：父元素的背景虽然画在正常流内容之下，
     却画在定位子元素的负层之上 —— 一写就把壁纸压没了（已经压没一次）。
     底色交给下面的 `.win-bg`，它和壁纸都按 DOM 顺序叠。 */
}

.win-bg {
  position: absolute;
  inset: 0;
  background: var(--xd-bg);
}

.shell {
  /* 压在底色和壁纸之上。这两层是绝对定位的，不给 `.shell` 提层的话，
     它们会跑到正常流内容上面去（定位元素默认盖住非定位元素）。 */
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: row;
  height: 100%;
  /* **这里不能有背景色**：`.shell` 是满屏容器，它的背景绘制在 z-index:-1 的壁纸层**之上**
     （正常流背景永远盖住负层），一写就把壁纸整个挡死 —— 表现是"选好了壁纸但看不出任何变化"。
     窗口底色由 `body` 提供（style.css 里 `body { background: var(--xd-bg) }`），一层就够。 */
}

/* 侧边栏右边的一切：标题栏 + 内容区。侧边栏是全高的，
   所以这一列自己竖排，标题栏不再横跨整个窗口宽度 */
.shell-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* 桌面通道没连上时的告警条：必须显眼，这时候保存是无效的 */
.env-note {
  flex: none;
  margin: 0;
  padding: 6px 12px;
  background: color-mix(in srgb, var(--xd-red) 16%, var(--xd-bg));
  border-bottom: 1px solid var(--xd-red);
  color: var(--xd-red);
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.5;
}

.shell-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
</style>
