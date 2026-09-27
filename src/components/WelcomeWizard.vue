<script setup lang="ts">
// 首次启动向导：介绍这软件能干嘛 → 引导装配套 AI 技能 → 说清数据在哪。
//
// **只弹一次**：靠配置里的 `welcome_done` 记着（关闭时由父组件写）。
// 它不是"一个工具"，所以不进侧边栏的 MainView，而是 Teleport 到 body 的浮层 ——
// 浮层留在有 transform 的容器里位置会跟着跑，这是项目里踩过的坑。
import { computed, ref } from 'vue'
import { Check, Copy, Github, ListChecks, Timer, Wallet } from 'lucide-vue-next'
import { configStore } from '../stores/config'
import { GITHUB_URL } from '../core/constants'

const emit = defineEmits<{ close: [] }>()

const step = ref(0)
const TOTAL = 3

// 装技能那段提示词。地址跟 `core/constants.ts` 里那两个常量同源 ——
// 改名时那边改完这里不用动（这里只放文字，链接是拼给人看的）
const INSTALL_TEXT = `帮我装一个 AI 技能：xiaodao-work（小刀工作台配套）
地址：${GITHUB_URL}

装到你的技能目录里（CodeBuddy / TRAE 是 ~/.agents/skills/xiaodao-work）。
装完告诉我一声，然后按这个技能里写的初始化提问问我几个问题
（主要是待办和记账的文件放哪），答完就能用了。`

const FEATURES = [
  { icon: ListChecks, name: '待办', desc: '你和 AI 共用的清单' },
  { icon: Timer, name: '番茄闹钟', desc: '专注一段，休息一段' },
  { icon: Github, name: 'GitHub 热榜', desc: '看看大家都在造什么' },
  { icon: Wallet, name: '记账', desc: '收支流水，按月汇总' },
]

const copyState = ref<'idle' | 'done' | 'fail'>('idle')
const copyErr = ref('')

async function copyInstall(): Promise<void> {
  try {
    await navigator.clipboard.writeText(INSTALL_TEXT)
    copyState.value = 'done'
    window.setTimeout(() => (copyState.value = 'idle'), 1500)
  } catch (e) {
    // 复制失败**必须说出来**：静默失败的话用户以为复制了，粘出来是空的，
    // 还会怪 AI 装不上 —— 这类"以为做了其实没做"的失败最坑
    copyState.value = 'fail'
    copyErr.value = String(e)
  }
}

const isLast = computed(() => step.value === TOTAL - 1)

function next(): void {
  if (!isLast.value) {
    step.value += 1
    return
  }
  emit('close')
}
</script>

<template>
  <Teleport to="body">
    <div class="ww-scrim" @click="emit('close')"></div>

    <div class="ww-card" role="dialog" aria-label="欢迎使用小刀工作台">
      <!-- 顶部：步骤点 + 跳过 -->
      <div class="ww-top">
        <div class="ww-dots">
          <span
            v-for="i in TOTAL"
            :key="i"
            class="ww-dot"
            :class="{ on: i - 1 === step, past: i - 1 < step }"
          ></span>
        </div>
        <button type="button" class="ww-skip" @click="emit('close')">跳过</button>
      </div>

      <!-- 第 1 页：这软件能干嘛 -->
      <div v-if="step === 0" key="intro" class="ww-body">
        <h3 class="ww-title">小刀工作台</h3>
        <p class="ww-sub">四个小工具装在左边侧边栏里，点一下就切过去。</p>
        <div class="ww-grid">
          <div v-for="f in FEATURES" :key="f.name" class="ww-feat">
            <component :is="f.icon" :size="20" :stroke-width="1.8" class="ww-feat-icon" />
            <span class="ww-feat-name">{{ f.name }}</span>
            <span class="ww-feat-desc">{{ f.desc }}</span>
          </div>
        </div>
      </div>

      <!-- 第 2 页：想让 AI 帮你管这些？ -->
      <div v-else-if="step === 1" key="skill" class="ww-body">
        <h3 class="ww-title">想让 AI 帮你管这些？</h3>
        <p class="ww-sub">
          装配套技能后，跟 AI 说一句就能加待办，发张账单截图它自己就记上账了。
        </p>
        <textarea class="ww-code" readonly :value="INSTALL_TEXT" rows="7"></textarea>
        <div class="ww-row">
          <button type="button" class="ww-btn primary" @click="copyInstall">
            <component
              :is="copyState === 'done' ? Check : Copy"
              :size="13"
              :stroke-width="2"
            />
            {{ copyState === 'done' ? '已复制' : '复制' }}
          </button>
          <span class="ww-hint">复制这段发给你常用的 AI，它就自己装好了</span>
        </div>
        <p v-if="copyState === 'fail'" class="ww-err">
          没复制成：{{ copyErr }}（也可以直接选中上面的文字手动复制）
        </p>
      </div>

      <!-- 第 3 页：数据在哪 -->
      <div v-else key="data" class="ww-body">
        <h3 class="ww-title">数据放在这</h3>
        <p class="ww-sub">待办、记账、设置都存在下面这个目录里，自动备份也在里面。</p>
        <div class="ww-path">{{ configStore.dataRoot !== '' ? configStore.dataRoot : '（还在读…）' }}</div>
        <p class="ww-hint">想换个地方随时能改：设置 → 数据 → 数据路径。</p>
      </div>

      <!-- 底部：上一步 / 下一步（开始使用） -->
      <div class="ww-foot">
        <button v-if="step > 0" type="button" class="ww-btn" @click="step -= 1">
          上一步
        </button>
        <span class="ww-spacer"></span>
        <button type="button" class="ww-btn primary" @click="next">
          {{ isLast ? '开始使用' : '下一步' }}
        </button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.ww-scrim {
  position: fixed;
  inset: 0;
  z-index: 80;
  /* 遮罩铺满视口，而窗口四角是透明的（圆角由 .win 裁出来）——
     不给同样的圆角，这两块会像补丁一样糊在窗外那四个角上 */
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.28);
}

.ww-card {
  position: fixed;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  z-index: 85;
  display: flex;
  flex-direction: column;
  width: min(420px, calc(100vw - 28px));
  max-height: calc(100vh - 28px);
  padding: 13px 16px 14px;
  border: 1px solid var(--xd-border);
  border-radius: 16px;
  background: var(--xd-sheet-bg);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(18px) saturate(1.4);
}

.ww-top {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.ww-dots {
  display: flex;
  gap: 5px;
}

.ww-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  border: 1px solid var(--xd-text-dim);
  transition: all 0.16s var(--xd-ease);
}

.ww-dot.past {
  background: var(--xd-text-dim);
}

.ww-dot.on {
  background: var(--xd-accent);
  border-color: var(--xd-accent);
}

.ww-skip {
  margin-left: auto;
  border: none;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(12.4px * var(--xd-font-scale));
  cursor: pointer;
}

.ww-skip:hover {
  color: var(--xd-text-sub);
}

.ww-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  animation: ww-in 0.16s var(--xd-ease);
}

@keyframes ww-in {
  from {
    opacity: 0;
    transform: translateX(6px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

.ww-title {
  margin: 0 0 3px;
  font-size: calc(15.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

.ww-sub {
  margin: 0 0 10px;
  font-size: calc(12.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  line-height: 1.5;
}

/* 四个功能卡 */
.ww-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 7px;
}

.ww-feat {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 9px 10px;
  border-radius: 10px;
  background: var(--xd-card-sub);
  border: 1px solid var(--xd-border-soft);
  transition: background 0.15s var(--xd-ease);
}

.ww-feat:hover {
  background: color-mix(in srgb, var(--xd-accent) 8%, var(--xd-card-sub));
}

.ww-feat-icon {
  color: var(--xd-accent);
}

.ww-feat-name {
  font-size: calc(13.2px * var(--xd-font-scale));
  font-weight: 500;
  color: var(--xd-text);
}

.ww-feat-desc {
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 提示词 */
.ww-code {
  width: 100%;
  padding: 8px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 9px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12.2px * var(--xd-font-scale));
  line-height: 1.5;
  resize: none;
  outline: none;
  user-select: text;
}

.ww-code:focus {
  border-color: var(--xd-accent);
}

.ww-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 7px;
}

.ww-hint {
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.ww-err {
  margin: 6px 0 0;
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-red);
}

.ww-path {
  padding: 7px 9px;
  border-radius: 9px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12.4px * var(--xd-font-scale));
  word-break: break-all;
  user-select: text;
}

.ww-foot {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 12px;
}

.ww-spacer {
  flex: 1;
}

.ww-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 13px;
  border: 1px solid var(--xd-border);
  border-radius: 9px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(13.2px * var(--xd-font-scale));
  cursor: pointer;
  transition: all 0.15s var(--xd-ease);
}

.ww-btn:hover {
  border-color: var(--xd-accent);
  color: var(--xd-text);
}

.ww-btn.primary {
  background: var(--xd-accent);
  border-color: var(--xd-accent);
  /* **不许写死 #fff**：暗色主题的强调色是荧光绿，很亮，压在上面的字必须是深色。
     这个变量在亮色下是白、暗色下是近黑 —— 全项目的主按钮都用它 */
  color: var(--xd-accent-text);
  font-weight: 600;
}

.ww-btn.primary:hover {
  background: var(--xd-accent-hover);
  border-color: var(--xd-accent-hover);
  color: var(--xd-accent-text);
}
</style>
