<script setup lang="ts">
// 归属切换：我的 / 小刀的。
// 角标是该归属下**未完成**的条数，不受搜索与筛选影响 —— 切筛选时数字不跳。
// 未加载完显示 `—` 而不是 0：0 会让人以为"真的一条都没有"。
import { OWNER_HINTS, OWNER_LABELS, OWNER_OPTIONS } from '../core/constants'
import type { Owner } from '../core/types'

const props = defineProps<{
  owner: Owner
  counts: Record<string, number>
  loaded: boolean
}>()

const emit = defineEmits<{ (e: 'change', owner: Owner): void }>()
</script>

<template>
  <div class="tabs" role="tablist">
    <button
      v-for="o in OWNER_OPTIONS"
      :key="o"
      type="button"
      role="tab"
      class="tab"
      :class="{ on: o === props.owner }"
      :aria-selected="o === props.owner"
      :title="OWNER_HINTS[o]"
      @click="emit('change', o)"
    >
      {{ OWNER_LABELS[o] }}
      <span class="tab-count">{{ props.loaded ? (props.counts[o] ?? 0) : '—' }}</span>
    </button>
  </div>
</template>

<style scoped>
/* 外圈是一块灰底"轨道"，选中的那一半是**实色填充**（原插件的口径）。
   别改回"浅底 + 描边"那种做法：一排灰底里，浅色选中项太弱，看不出当前在哪一栏。 */
.tabs {
  display: flex;
  gap: 3px;
  padding: 3px;
  border-radius: 11px;
  background: var(--xd-card-sub);
}

.tab {
  flex: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: calc(30px * var(--xd-font-scale));
  padding: 0 8px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--xd-text-sub);
  font-size: calc(15px * var(--xd-font-scale));
  font-weight: 500;
  cursor: pointer;
  transition:
    background 0.16s var(--xd-ease),
    color 0.16s var(--xd-ease);
}

.tab:hover {
  background: var(--xd-hover);
}

.tab.on {
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
  box-shadow: 0 1px 3px rgba(16, 24, 40, 0.18);
}

.tab.on:hover {
  filter: brightness(1.05);
}

/* 角标不单独配色，跟着所在按钮的文字色走（选中时就是反白色）—— 原插件也是这样 */
.tab-count {
  font-size: 0.92em;
  opacity: 0.85;
  font-variant-numeric: tabular-nums;
}
</style>
