<script setup lang="ts">
// 筛选抽屉：领域 + 权重 + 已完成开关。
//
// chip 上的数字**不含本字段自身的筛选**（选了"高"，"中""低"的数字不变）——
// 否则一点就把别的选项变成 0，看着像没数据了，还以为筛错了。
import { computed, onBeforeUnmount, watch } from 'vue'
import { X } from 'lucide-vue-next'
import { PRIORITY_LABELS, PRIORITY_OPTIONS } from '../core/constants'
import { useDomains } from '../composables/useDomains'

const { options: domainOptions, label: domainName } = useDomains()
import { countBy } from '../core/view'
import type { TodoItem } from '../core/types'

const props = defineProps<{
  open: boolean
  pool: readonly TodoItem[]
  domain: string | null
  priority: string | null
}>()

const emit = defineEmits<{
  (e: 'update:domain', v: string | null): void
  (e: 'update:priority', v: string | null): void
  (e: 'close'): void
}>()

const domainCounts = computed(() => countBy(props.pool, 'domain'))
const priorityCounts = computed(() => countBy(props.pool, 'priority'))

/** Esc 关掉。浮层本来就该能被键盘收掉 —— 鼠标点在遮罩上是另一条常用退路 */
function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

// 只在开着的时候挂键盘监听：关着的时候没必要跟着每一次按键跑
watch(
  () => props.open,
  (open) => {
    if (open) window.addEventListener('keydown', onKey)
    else window.removeEventListener('keydown', onKey)
  },
)
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

function toggleDomain(d: string): void {
  emit('update:domain', props.domain === d ? null : d)
}

function togglePriority(p: string): void {
  emit('update:priority', props.priority === p ? null : p)
}
</script>

<template>
  <!-- 传送到 body：卡片 hover 有 `transform`、外卡还有圆角裁剪 ——
       留在原地的话 `fixed` 的基准会变成卡片（位置跟着卡片跑），还会被裁掉。
       改成画面中间的浮层之后，筛选的内容也不再被列表挡住。 -->
  <Teleport to="body">
    <template v-if="props.open">
      <div class="fs-scrim" @click="emit('close')" />
      <div class="fs-sheet" role="dialog" aria-label="筛选" @click.stop>
        <div class="sheet-head">
      <span class="sheet-title">筛选</span>
      <button type="button" class="sheet-x" aria-label="关闭" @click="emit('close')">
        <X :size="14" :stroke-width="2" />
      </button>
    </div>

    <p class="sheet-label">领域</p>
    <div class="sheet-chips">
      <button
        type="button"
        class="sheet-chip"
        :class="{ on: props.domain === null }"
        @click="emit('update:domain', null)"
      >
        全部
      </button>
      <button
        v-for="d in domainOptions"
        :key="d"
        type="button"
        class="sheet-chip"
        :class="{ on: props.domain === d }"
        @click="toggleDomain(d)"
      >
        {{ domainName(d) }}
        <span class="sheet-n">{{ domainCounts[d] ?? 0 }}</span>
      </button>
    </div>

    <p class="sheet-label">权重</p>
    <div class="sheet-chips">
      <button
        type="button"
        class="sheet-chip"
        :class="{ on: props.priority === null }"
        @click="emit('update:priority', null)"
      >
        全部
      </button>
      <button
        v-for="p in PRIORITY_OPTIONS"
        :key="p"
        type="button"
        class="sheet-chip"
        :class="[`w-${p}`, { on: props.priority === p }]"
        @click="togglePriority(p)"
      >
        {{ PRIORITY_LABELS[p] }}
        <span class="sheet-n">{{ priorityCounts[p] ?? 0 }}</span>
      </button>
    </div>

        <div class="sheet-actions">
          <button type="button" class="sheet-btn ghost" @click="emit('close')">取消</button>
        </div>
        <p class="fs-hint">点遮罩或按 Esc 也可以关闭</p>
      </div>
    </template>
  </Teleport>
</template>

<style scoped>
/* 遮罩铺满视口。窗口四角是透明的（圆角由 .win 裁出来），
   不给遮罩同样的圆角，它就会像补丁一样糊在窗外那四个角上 */
.fs-scrim {
  position: fixed;
  inset: 0;
  z-index: 65;
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.35);
}

/* 居中浮层。底色与模糊参数跟「完整表单」「删除确认」**共用同一组变量** ——
   三处一起改，不会出现"三个浮层像三个软件"的情况 */
.fs-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  translate: -50% -50%;
  z-index: 70;
  width: min(360px, calc(100vw - 40px));
  max-height: calc(100vh - 60px);
  overflow-y: auto;
  padding: 14px 16px 12px;
  border-radius: var(--xd-radius-lg);
  background: var(--xd-sheet-bg);
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
  animation: fs-in 0.16s var(--xd-ease);
}

@keyframes fs-in {
  from {
    opacity: 0;
    translate: -50% -47%;
  }
}

.fs-hint {
  margin: 6px 0 0;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.sheet-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.sheet-title {
  font-size: calc(15.6px * var(--xd-font-scale));
  font-weight: 600;
}

.sheet-x {
  display: grid;
  place-items: center;
  width: calc(22px * var(--xd-font-scale));
  height: calc(22px * var(--xd-font-scale));
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--xd-text-dim);
}

.sheet-x:hover {
  background: var(--xd-card-sub);
  color: var(--xd-text);
}

.sheet-label {
  margin: 8px 0 5px;
  color: var(--xd-text-dim);
  font-size: calc(12.6px * var(--xd-font-scale));
}

.sheet-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.sheet-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: transparent;
  color: var(--xd-text-sub);
  font-size: calc(13.8px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.sheet-chip:hover {
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}

.sheet-chip.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}

.sheet-chip.on.w-high {
  border-color: var(--xd-w-high);
  background: color-mix(in srgb, var(--xd-w-high) 13%, transparent);
  color: var(--xd-w-high);
}

.sheet-chip.on.w-mid {
  border-color: var(--xd-w-mid);
  background: color-mix(in srgb, var(--xd-w-mid) 13%, transparent);
  color: var(--xd-w-mid);
}

.sheet-chip.on.w-low {
  border-color: var(--xd-w-low);
  background: color-mix(in srgb, var(--xd-w-low) 15%, transparent);
  color: var(--xd-text-sub);
}

.sheet-n {
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12px * var(--xd-font-scale));
  opacity: 0.75;
}

.sheet-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 12px;
  padding-top: 10px;
  border-top: 1px solid var(--xd-border-soft);
}

.sheet-info {
  display: flex;
  flex-direction: column;
}

.sheet-name {
  font-size: calc(14.4px * var(--xd-font-scale));
  color: var(--xd-text);
}

.sheet-desc {
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 开关：与设置页共用一套观感 */
.toggle {
  flex: none;
  position: relative;
  width: 36px;
  height: 20px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: var(--xd-card-sub);
  transition: all 0.18s var(--xd-ease);
}

.toggle.on {
  background: var(--xd-accent);
  border-color: var(--xd-accent);
}

.toggle-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--xd-text-dim);
  transition: all 0.18s var(--xd-ease);
}

.toggle.on .toggle-knob {
  left: 18px;
  background: #fff;
}

.sheet-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.sheet-btn {
  flex: 1;
  padding: 6px 10px;
  border-radius: 8px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
}

.sheet-btn:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}
</style>
