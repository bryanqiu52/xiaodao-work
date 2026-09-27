<script setup lang="ts">
// 已完成行：一行一个，标题单行省略，完成日靠右。
// 悬停才浮出「恢复 / 删除」—— 已完成是归档区，不需要常驻的操作按钮抢视线。
import { computed } from 'vue'
import { CheckCircle2, RotateCcw, Trash2 } from 'lucide-vue-next'
import IdChip from './IdChip.vue'
import type { TodoItem } from '../core/types'

const props = defineProps<{
  item: TodoItem
  /** 显示归属标签。只有全局搜索才需要 —— 它跨归属出结果，不标会分不清是谁的事 */
  showOwner?: boolean
}>()
const emit = defineEmits<{ (e: 'restore'): void; (e: 'remove'): void }>()

const doneText = computed(() => {
  const d = props.item.doneAt
  if (!d) return ''
  const date = new Date(d)
  if (Number.isNaN(date.getTime())) return ''
  const p = (n: number): string => (n < 10 ? `0${n}` : String(n))
  return `${p(date.getMonth() + 1)}-${p(date.getDate())}`
})
</script>

<template>
  <div class="done" :class="{ 'is-deleted': props.item.deletedAt }">
    <CheckCircle2 :size="14" :stroke-width="2.2" class="done-mark" />
    <IdChip :id="props.item.id" />
    <span class="done-title xd-select" :title="props.item.title">{{ props.item.title }}</span>
    <span v-if="props.showOwner" class="owner-tag">
      {{ props.item.owner === 'agent' ? '小刀的' : '我的' }}
    </span>
    <span v-if="props.item.deletedAt" class="deleted-tag">已删除</span>
    <span class="done-date">{{ doneText }}</span>
    <div class="done-ops">
      <button type="button" class="done-op" title="恢复成未完成" @click.stop="emit('restore')">
        <RotateCcw :size="13" :stroke-width="2" />
      </button>
      <button type="button" class="done-op danger" title="移出列表" @click.stop="emit('remove')">
        <Trash2 :size="13" :stroke-width="2" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.done {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 7px 11px;
  border-radius: 10px;
  border: 1px solid var(--xd-border-soft);
  background: var(--xd-card-sub);
  transition: background 0.16s var(--xd-ease), border-color 0.16s var(--xd-ease);
}

.done:hover {
  background: var(--xd-card);
  border-color: var(--xd-border);
}

/* 已删除：跟普通卡片一个口径（压暗 + 标签）。
   软删除的记录一直都在，默认不显示；打开「显示已删除」后才冒出来，
   所以必须一眼能认出它是已被移出的，否则跟正常条目混在一起分不清。 */
.done.is-deleted {
  opacity: 0.55;
}

.owner-tag {
  flex: none;
  padding: 1px 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--xd-text) 8%, transparent);
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.deleted-tag {
  flex: none;
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px dashed var(--xd-border);
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.done-mark {
  flex: none;
  color: var(--xd-green);
}

.done-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: calc(15px * var(--xd-font-scale));
  color: var(--xd-text-sub);
}

.done-date {
  flex: none;
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  font-family: 'Cascadia Code', Consolas, monospace;
}

.done-ops {
  flex: none;
  display: flex;
  gap: 1px;
  opacity: 0;
  transition: opacity 0.16s var(--xd-ease);
}

.done:hover .done-ops,
.done:focus-within .done-ops {
  opacity: 1;
}

.done-op {
  display: grid;
  place-items: center;
  width: calc(22px * var(--xd-font-scale));
  height: calc(22px * var(--xd-font-scale));
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--xd-text-dim);
  transition: all 0.14s var(--xd-ease);
}

.done-op:hover {
  background: var(--xd-card-sub);
  color: var(--xd-text);
}

.done-op.danger:hover {
  color: var(--xd-red);
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
}
</style>
