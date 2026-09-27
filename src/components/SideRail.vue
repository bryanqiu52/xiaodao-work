<script setup lang="ts">
// 左侧窄图标导航：待办 / 番茄闹钟 / GitHub 热榜 / 记账，底部设置。
//
// 窄栏（52px）只放图标不放文字 —— 文字标签走悬停 tooltip（纯 CSS）；
// 选中态用强调色底 + 图标着色，跟设置页分段按钮（.sv-btn.on）同一个语言。
// 交互参考了 x-hub 的侧边栏：原生 button（天然键盘可达）、aria-current、
// tooltip 延迟 0.25s 淡入（扫过不闪、停下来才出）。
import { computed, type Component } from 'vue'
import {
  BarChart3,
  Github,
  ListTodo,
  Moon,
  Settings as SettingsIcon,
  Sun,
  Timer,
  Wallet,
} from 'lucide-vue-next'
import { resolveTheme } from '../core/config'
import { configStore, saveConfig } from '../stores/config'

export type MainView = 'todo' | 'focus' | 'github' | 'money' | 'review' | 'settings'

const props = defineProps<{ view: MainView }>()
const emit = defineEmits<{ (e: 'navigate', v: MainView): void }>()

/**
 * 当前**实际生效**的主题（`system` 要解开成 light / dark 才知道现在到底是哪个）。
 * 图标显示的是"按下去会变成什么"的反面：暗的时候给太阳，亮的时候给月亮。
 */
const isDark = computed(() => resolveTheme(configStore.cfg.theme_mode) === 'dark')

/** 一键切换亮 / 暗。跟随系统的话按下去就定死在当前这一档 —— 手动选过就该听手动的 */
async function toggleTheme(): Promise<void> {
  await saveConfig({ theme_mode: isDark.value ? 'light' : 'dark' })
}

const items: { id: Exclude<MainView, 'settings'>; label: string; icon: Component }[] = [
  { id: 'todo', label: '待办', icon: ListTodo },
  { id: 'focus', label: '番茄闹钟', icon: Timer },
  { id: 'github', label: 'GitHub 热榜', icon: Github },
  { id: 'money', label: '记账', icon: Wallet },
  // 回顾放最后：它是"回头看"的入口，不是每天开工第一站
  { id: 'review', label: '回顾', icon: BarChart3 },
]
</script>

<template>
  <nav class="rail" aria-label="工具导航">
    <button
      v-for="it in items"
      :key="it.id"
      type="button"
      class="rail-btn"
      :class="{ on: props.view === it.id }"
      :aria-current="props.view === it.id ? 'page' : undefined"
      :data-tip="it.label"
      @click="emit('navigate', it.id)"
    >
      <component :is="it.icon" :size="19" :stroke-width="2" />
    </button>

    <!-- 主题切换：图标显示的是"按下去会变成什么样"的反面 ——
         暗的时候给太阳（去亮色），亮的时候给月亮（去暗色） -->
    <button
      type="button"
      class="rail-btn rail-theme"
      :data-tip="isDark ? '切换到亮色' : '切换到暗色'"
      :title="isDark ? '切换到亮色' : '切换到暗色'"
      @click="toggleTheme"
    >
      <Sun v-if="isDark" :size="19" :stroke-width="2" />
      <Moon v-else :size="19" :stroke-width="2" />
    </button>

    <button
      type="button"
      class="rail-btn rail-settings"
      :class="{ on: props.view === 'settings' }"
      :aria-current="props.view === 'settings' ? 'page' : undefined"
      data-tip="设置"
      @click="emit('navigate', 'settings')"
    >
      <SettingsIcon :size="19" :stroke-width="2" />
    </button>
  </nav>
</template>

<style scoped>
.rail {
  flex: none;
  width: 52px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 8px 0 10px;
  /* 一层很薄的底：完全透明的话，浅色壁纸上灰图标认不出来；
     但不能厚 —— 沉浸模式靠的就是透，厚了把壁纸糊死（壁纸被盖住这事踩过两次了） */
  background: color-mix(in srgb, var(--xd-bg) 30%, transparent);
  border-right: 1px solid var(--xd-border);
}

.rail-btn {
  position: relative;
  flex: none;
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 11px;
  border: 1px solid transparent;
  background: transparent;
  /* 用次档而不是最弱那档：19px 的描边图标比文字更吃对比度，
     亮色下用 dim 会淡到认不出是哪个图标（2026-09-25 修） */
  color: var(--xd-text-sub);
  cursor: pointer;
  transition:
    background 0.15s ease-out,
    color 0.15s ease-out,
    border-color 0.15s ease-out;
}

.rail-btn:hover {
  background: color-mix(in srgb, var(--xd-text) 8%, transparent);
  color: var(--xd-text);
}

.rail-btn.on {
  background: var(--xd-accent-soft);
  border-color: color-mix(in srgb, var(--xd-accent) 35%, transparent);
  color: var(--xd-accent);
}

/* 窄栏没地方放文字，标签悬停时从右侧弹出。
   底色用浮层色（--xd-sheet-bg）：不透明，保证压在壁纸上也看得清 */
.rail-btn::after {
  content: attr(data-tip);
  position: absolute;
  left: calc(100% + 10px);
  top: 50%;
  translate: 0 -50%;
  padding: 4px 9px;
  border-radius: 8px;
  background: var(--xd-sheet-bg);
  border: 1px solid var(--xd-border);
  color: var(--xd-text);
  font-size: calc(12.4px * var(--xd-font-scale));
  white-space: nowrap;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease-out 0.25s;
  z-index: 30;
}

.rail-btn:hover::after {
  opacity: 1;
}

/* 主题切换与设置压在底部一组：主工具在顶部，一眼分清"常用"和"低频"。
   `margin-top: auto` 挂在主题这一颗上，它把整组顶到底，设置跟在它下面 */
.rail-theme {
  margin-top: auto;
}

.rail-settings {
  margin-top: 0;
}
</style>
