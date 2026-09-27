<script setup lang="ts">
// 「发现新版本」浮层。数据与动作都在 `composables/useUpdate.ts`，这里只管画。
//
// **说明文字刻意不做真正的 markdown 渲染**：只把 `##` 去掉、`-` 换成「·」、
// 去掉 `**`，然后用 `pre-wrap` 原样排。理由是踩过的一次教训 ——
// 系统原生确认框不认 markdown，我们写在里面的 `**粗体**` 连着星号显示了很久。
// 结论是"给渲染器写文案之前先确认它认什么"：这里的渲染器是我们自己的，
// 那就把它的能力定死，然后让 release 说明按这个格式写（规则见 RELEASE.md）。
import { computed, ref } from 'vue'
import { Download, Loader2, Sparkles } from 'lucide-vue-next'
import { dismissUpdate, installUpdate, updateState } from '../composables/useUpdate'
import { releaseUrl } from '../core/constants'
import { isTauri, tauriApi } from '../api/tauri'

const visible = computed(() => updateState.info !== null && !updateState.dismissed)

/** 跳 release 页失败时的提示（跟"更新失败"分开，各说各的） */
const linkErr = ref('')

/**
 * 跳到 Release 页面。
 *
 * 用 `tauriApi.openUrl`（走 Rust 的 `open_url`，只放行 http/https）而不是
 * `<a target="_blank">` —— webview 里点链接的行为不稳，也不受我们的白名单管。
 */
async function openRelease(): Promise<void> {
  const v = updateState.info?.version ?? ''
  if (v.length === 0) return
  linkErr.value = ''
  if (!isTauri()) {
    linkErr.value = `浏览器调试模式下请手动打开：${releaseUrl(v)}`
    return
  }
  const err = await tauriApi.openUrl(releaseUrl(v))
  if (err !== null) linkErr.value = `没打开浏览器：${err}`
}

const notes = computed(() => {
  const raw = updateState.info?.body ?? ''
  if (raw.trim().length === 0) return '（这一版没有写说明）'
  return raw
    .split(/\r?\n/)
    .map((line) => line.replace(/^#{1,6}\s*/, '')) // 标题去掉井号，当普通行
    .map((line) => line.replace(/^\s*[-*]\s+/, '· ')) // 列表项换成「·」，纯文本里也读得下去
    .map((line) => line.replace(/\*\*(.+?)\*\*/g, '$1')) // 粗体星号去掉，别让它当字面量显示
    .join('\n')
    .trim()
})

const pct = computed(() => updateState.progress)
const busy = computed(() => updateState.downloading)

/** 下载中不给关：关了用户就看不到进度，还以为没在装 */
function onScrim(): void {
  if (busy.value) return
  void dismissUpdate()
}

function onKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || busy.value) return
  void dismissUpdate()
}
</script>

<template>
  <!-- 传送到 body：卡片 hover 有 transform，留在里面 fixed 的基准会变成卡片 -->
  <Teleport to="body">
    <template v-if="visible">
      <div class="us-scrim" @click="onScrim" />
      <div class="us-sheet" role="dialog" aria-label="发现新版本" tabindex="-1" @click.stop @keydown="onKey">
        <p class="us-title">
          <Sparkles :size="14" :stroke-width="2.2" class="us-icon" />
          发现新版本 v{{ updateState.info?.version }}
        </p>
        <p v-if="updateState.info?.date" class="us-date">{{ updateState.info.date }}</p>

        <p class="us-notes">{{ notes }}</p>

        <!-- 下载中：进度条 + 一句人话。总大小拿不到时（pct 为负）不编百分比 -->
        <div v-if="busy" class="us-progress">
          <div class="us-bar">
            <i v-if="pct >= 0" :style="{ width: `${pct}%` }" />
            <i v-else class="indeterminate" />
          </div>
          <span class="us-progress-text">
            {{ pct >= 0 ? `正在下载 ${pct}%` : '正在下载…' }}
          </span>
        </div>

        <p v-if="updateState.error" class="us-error">更新没成功：{{ updateState.error }}</p>
        <p v-else-if="linkErr" class="us-error">{{ linkErr }}</p>

        <div class="us-actions">
          <button type="button" class="us-btn ghost" :disabled="busy" @click="dismissUpdate">
            这次先不更新
          </button>
          <button type="button" class="us-btn primary" :disabled="busy" @click="installUpdate">
            <Loader2 v-if="busy" :size="13" :stroke-width="2.2" class="us-spin" />
            <Download v-else :size="13" :stroke-width="2.2" />
            {{ busy ? '正在更新' : '立即更新' }}
          </button>
        </div>

        <p class="us-hint">
          装完会自动重启。也可以
          <button type="button" class="us-link" @click="openRelease">去 Release 页面手动下</button>
          （按 Esc 也是"先不更新"，同一版不会再打扰你）
        </p>
      </div>
    </template>
  </Teleport>
</template>

<style scoped>
.us-scrim {
  position: fixed;
  inset: 0;
  z-index: 78;
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.35);
}

.us-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  translate: -50% -50%;
  /* 比确认浮层低一层：万一更新弹着的时候又冒出个确认框，它该在上面 */
  z-index: 82;
  display: flex;
  flex-direction: column;
  gap: 7px;
  width: min(400px, calc(100vw - 32px));
  max-height: calc(100vh - 40px);
  padding: 16px 18px 14px;
  border-radius: var(--xd-radius-lg);
  background: var(--xd-sheet-bg);
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
  animation: us-in 0.18s var(--xd-ease);
}

@keyframes us-in {
  from {
    opacity: 0;
    translate: -50% -47%;
  }
}

.us-title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: calc(15.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

.us-icon {
  flex: none;
  color: var(--xd-accent);
}

.us-date {
  margin: -4px 0 0;
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 说明：等宽 + 保留换行。`pre-wrap` 让长行自己折，不用逐行拆元素 */
.us-notes {
  margin: 2px 0 0;
  max-height: min(280px, 42vh);
  overflow-y: auto;
  padding: 8px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--xd-text) 5%, transparent);
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12.4px * var(--xd-font-scale));
  line-height: 1.6;
  color: var(--xd-text-sub);
  white-space: pre-wrap;
  word-break: break-word;
}

.us-progress {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 2px;
}

.us-bar {
  height: calc(6px * var(--xd-font-scale));
  border-radius: 999px;
  background: var(--xd-border-soft);
  overflow: hidden;
}

.us-bar i {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--xd-accent);
  transition: width 0.2s linear;
}

/* 总大小未知：一条来回跑的条，别假装知道进度 */
.us-bar i.indeterminate {
  width: 35%;
  animation: us-slide 1.1s ease-in-out infinite;
}

@keyframes us-slide {
  0% {
    margin-left: -35%;
  }
  100% {
    margin-left: 100%;
  }
}

.us-progress-text {
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  font-variant-numeric: tabular-nums;
}

.us-error {
  margin: 2px 0 0;
  padding: 6px 9px;
  border-radius: 7px;
  background: color-mix(in srgb, var(--xd-red) 10%, transparent);
  font-size: calc(12.2px * var(--xd-font-scale));
  line-height: 1.5;
  color: var(--xd-red);
}

.us-actions {
  display: flex;
  gap: 8px;
  margin-top: 3px;
}

.us-btn {
  flex: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.us-btn.ghost:hover:not(:disabled) {
  border-color: var(--xd-text-dim);
  color: var(--xd-text);
}

.us-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.us-btn.primary:hover:not(:disabled) {
  background: var(--xd-accent-hover);
}

.us-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.us-spin {
  animation: us-rotate 1s linear infinite;
}

@keyframes us-rotate {
  to {
    transform: rotate(360deg);
  }
}

.us-hint {
  margin: 0;
  font-size: calc(11.6px * var(--xd-font-scale));
  line-height: 1.55;
  color: var(--xd-text-dim);
}

/* 看着像链接，其实是 button（见 openRelease 的说明）：清掉按钮默认长相 */
.us-link {
  padding: 0;
  border: none;
  background: none;
  color: var(--xd-accent);
  font-size: inherit;
  font-family: inherit;
  text-decoration: none;
  cursor: pointer;
}

.us-link:hover {
  text-decoration: underline;
}
</style>
