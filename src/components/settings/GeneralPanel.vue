<script setup lang="ts">
// 常规：快捷键、关窗行为、窗口置顶、开机自启。
import { computed, inject, onBeforeUnmount, onMounted, ref } from 'vue'
import { tauriApi, isTauri } from '../../api/tauri'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { configStore, saveConfig } from '../../stores/config'
import { useShortcutRecorder } from '../../composables/useShortcutRecorder'

const showToast = inject<(s: string) => void>('showToast', () => {})
const win = isTauri() ? getCurrentWindow() : null

/**
 * 快捷键**实际**注册上了没。
 * 光有输入框不够：录进去的组合可能被别的程序占着，那它一次都不会生效 ——
 * 用户会以为"功能坏了"，其实是没抢到。得把真实状态摆出来。
 */
const shortcutState = ref('')

async function refreshShortcutState(): Promise<void> {
  shortcutState.value = await tauriApi.shortcutState()
}

const shortcutNote = computed(() => {
  switch (shortcutState.value) {
    case '':
    case 'ok':
      return ''
    case 'conflict':
      return '这个组合被别的程序占用了，换一个（当前快捷键不会生效）'
    case 'invalid':
      return '这个组合系统不认，换一个再试'
    case 'not-registered':
      return '还没注册上（启动时可能被占用）'
    default:
      return `注册没成功：${shortcutState.value}`
  }
})

const recorder = useShortcutRecorder({
  initial: configStore.cfg.global_shortcut,
  save: async (v) => {
    const r = await tauriApi.setShortcut(v)
    // 录完立刻回查：改完还是冲突的话，提示要马上跟着变
    await refreshShortcutState()
    return r
  },
  showToast,
})

const keyInputRef = ref<HTMLInputElement | null>(null)

/**
 * 开机自启。**状态不进 config，每次直接问系统** ——
 * 用户可能从任务管理器里把它关掉，存一份就会出现"界面显示开着、其实早关了"。
 * 这和上面快捷键那条是同一个道理：显示的东西必须是真的。
 */
const autostart = ref<{ enabled: boolean; exe: string; debug: boolean }>({
  enabled: false,
  exe: '',
  debug: false,
})
const autostartBusy = ref(false)

async function refreshAutostart(): Promise<void> {
  autostart.value = await tauriApi.autostartInfo()
}

async function toggleAutostart(): Promise<void> {
  if (autostartBusy.value) return
  autostartBusy.value = true
  const next = !autostart.value.enabled
  const err = await tauriApi.setAutostart(next)
  autostartBusy.value = false
  if (err !== null) {
    showToast(`设置失败：${err}`)
    return
  }
  // 回查而不是本地翻转：万一系统那边没落上，本地翻转就骗了自己
  await refreshAutostart()
  showToast(next ? '已开启开机自启' : '已关闭开机自启')
}

/**
 * 开发版下自启指向的是临时产物，**不管开没开都提示** ——
 * 等开了才发现指向 debug exe 就晚了，用户会以为"正式版也一直这样"。
 * 带上实际路径，用户能在注册表/任务管理器里对上号。
 */
const autostartNote = computed(() => {
  if (!autostart.value.debug) return ''
  const exe = autostart.value.exe
  const where = exe.length > 0 ? exe : 'target\\debug 里的 exe'
  return `当前是开发版：自启会指向 ${where}，那是临时产物（删了就失效）。正式使用请出包后再开`
})

// ── 通知预览 ──────────────────────────────────────────────────────────────

const notifyBusy = ref(false)

/**
 * 发一条**真的**系统通知（不是界面里的假弹窗）。
 * 失败原因要说出来：通知被系统拦掉是最常见的一类"提醒没来"，用户得能查到。
 */
async function previewRemind(): Promise<void> {
  notifyBusy.value = true
  const err = await tauriApi.notify('待办到期提醒', '把会议记录整理成一页结论（今天到期）')
  notifyBusy.value = false
  showToast(err === null ? '已发一条，看屏幕右下角的通知中心' : `没发出来：${err}`)
}

async function previewFocus(): Promise<void> {
  notifyBusy.value = true
  const err = await tauriApi.notify('番茄完成', '专注 25 分钟完成，休息一下')
  notifyBusy.value = false
  showToast(err === null ? '已发一条，看屏幕右下角的通知中心' : `没发出来：${err}`)
}

onMounted(() => {
  window.addEventListener('keydown', recorder.onKeydown, true)
  void refreshShortcutState()
  void refreshAutostart()
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', recorder.onKeydown, true)
})

async function setCloseBehavior(v: string): Promise<void> {
  await saveConfig({ close_behavior: v })
  showToast(v === 'hide' ? '关闭窗口时藏到托盘' : '关闭窗口时直接退出')
}

async function toggleEdgeHide(): Promise<void> {
  const next = !configStore.cfg.edge_hide
  await saveConfig({ edge_hide: next })
  showToast(next ? '拖到屏幕边缘会自动收起' : '已关掉贴边隐藏')
}

async function togglePin(): Promise<void> {
  const next = !configStore.cfg.always_on_top
  await saveConfig({ always_on_top: next })
  if (win) {
    try {
      await win.setAlwaysOnTop(next)
    } catch (e) {
      console.error('[小刀工作台] 置顶失败', e)
    }
  }
}
</script>

<template>
  <!-- 快捷键 -->
  <section id="sv-sec-shortcut" class="sv-sec">
    <h3 class="sv-sec-title">快捷键</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">全局快捷键</span>
        <span class="setting-desc">
          一键唤起 / 隐藏窗口。点「录入」后直接按组合键（修饰键在前，如 Ctrl+Shift+Space）
        </span>
        <span v-if="recorder.error.value" class="setting-soon">{{ recorder.error.value }}</span>
        <span v-else-if="shortcutNote" class="setting-soon">{{ shortcutNote }}</span>
      </div>
      <div class="sv-swatches">
        <input
          ref="keyInputRef"
          class="sv-input"
          style="width: 132px"
          type="text"
          readonly
          :value="recorder.listening.value ? '按下组合键…' : recorder.value.value"
          :placeholder="configStore.cfg.global_shortcut"
          @click="recorder.startListening()"
          @blur="recorder.onBlur()"
        />
        <button type="button" class="sv-btn" @click="recorder.startListening()">录入</button>
      </div>
    </div>

  </section>

  <!-- 窗口 -->
  <section id="sv-sec-window" class="sv-sec">
    <h3 class="sv-sec-title">窗口</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">关闭窗口时</span>
        <span class="setting-desc">默认藏到托盘，随时按快捷键叫出来</span>
      </div>
      <div class="sv-seg">
        <button
          type="button"
          :class="{ on: configStore.cfg.close_behavior === 'hide' }"
          @click="setCloseBehavior('hide')"
        >
          藏到托盘
        </button>
        <button
          type="button"
          :class="{ on: configStore.cfg.close_behavior === 'quit' }"
          @click="setCloseBehavior('quit')"
        >
          退出
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">窗口置顶</span>
        <span class="setting-desc">一直浮在最上层</span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.always_on_top"
        :class="{ on: configStore.cfg.always_on_top }"
        @click="togglePin"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">贴边自动隐藏</span>
        <span class="setting-desc">
          窗口拖到屏幕左右边缘停住后自动收起，鼠标划到边缘再滑出来；嫌它碍事就关掉
        </span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.edge_hide"
        :class="{ on: configStore.cfg.edge_hide }"
        @click="toggleEdgeHide"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>

  </section>

  <!-- 启动 -->
  <section id="sv-sec-startup" class="sv-sec">
    <h3 class="sv-sec-title">启动</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">开机自动启动</span>
        <span class="setting-desc">开机后在托盘待命，不弹窗</span>
        <span v-if="autostartNote" class="setting-soon">{{ autostartNote }}</span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :disabled="autostartBusy"
        :aria-checked="autostart.enabled"
        :class="{ on: autostart.enabled }"
        @click="toggleAutostart"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>
  </section>

  <!-- 通知预览：到期提醒和番茄钟的通知都是"到点了才发"，
       平时根本看不到长什么样，这里能随手发一条真的 -->
  <section id="sv-sec-notify" class="sv-sec">
    <h3 class="sv-sec-title">通知预览</h3>
    <p class="sv-sec-note">
      这两种通知平时看不到。点「发一条」会真的发一条，顺带确认系统没拦掉。
    </p>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">到期提醒的通知</span>
        <span class="setting-desc">单条写清标题与到期日，多条会合并成一条</span>
      </div>
      <button type="button" class="sv-btn" :disabled="notifyBusy" @click="previewRemind">
        发一条
      </button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">番茄钟的通知</span>
        <span class="setting-desc">专注结束 / 休息结束各一条，文字不同</span>
      </div>
      <button type="button" class="sv-btn" :disabled="notifyBusy" @click="previewFocus">
        发一条
      </button>
    </div>
  </section>
</template>
