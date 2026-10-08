<script setup lang="ts">
// 状态 chip：点它直接改状态（走 applyPatch 那个唯一入口，会自动留流水）。
// 两个视图都有 —— 状态是通用的，「待回复」可以是小刀在等你，也可以是你在等客户回话。
// 顺序按点击频率排：暂停 / 排队最常点，顶在最前。
import { STATUS_ACTION_LABELS, STATUS_OPTIONS } from '../core/constants'
import type { Status } from '../core/types'

const props = defineProps<{ status: Status }>()
const emit = defineEmits<{ (e: 'change', next: Status): void }>()
</script>

<template>
  <div class="sb" @click.stop>
    <button
      v-for="s in STATUS_OPTIONS"
      :key="s"
      type="button"
      class="sb-chip"
      :class="[`st-${s}`, { on: s === props.status }]"
      :aria-pressed="s === props.status"
      @click="emit('change', s)"
    >
      {{ STATUS_ACTION_LABELS[s] ?? s }}
    </button>
  </div>
</template>

<style scoped>
.sb {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.sb-chip {
  padding: 2px 7px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: calc(19.2px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.sb-chip:hover {
  background: var(--xd-hover);
}

/* 选中态是**实色填充**，且一律用强调色（原插件的口径）。
   以前按状态各配一色，一排里红的蓝的橙的都有，反而看不出"当前是哪一档"；
   品牌色只有一个含义：这就是现在选中的那个。 */
.sb-chip.on {
  border-color: transparent;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.sb-chip.on:hover {
  filter: brightness(1.06);
}
</style>
