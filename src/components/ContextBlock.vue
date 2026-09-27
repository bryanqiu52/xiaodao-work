<script setup lang="ts">
// 脉络：正文 / 来源 / 关联待办 / 关联文件 / 流水。
//
// 卡片上只留一句概要，其余全折进这里 —— 点开才展开，避免一张卡占满半个屏幕。
import { computed, ref } from 'vue'
import { ChevronRight } from 'lucide-vue-next'
import DetailText from './DetailText.vue'
import IdChip from './IdChip.vue'
import ArtifactChip from './ArtifactChip.vue'
import type { TodoItem } from '../core/types'

const props = defineProps<{ item: TodoItem; libRoot: string }>()
const emit = defineEmits<{ (e: 'result', text: string): void }>()

const open = ref(false)

const trail = computed(() => [...props.item.trail].slice().reverse())
const hasOrigin = computed(
  () => props.item.origin.from.length > 0 || props.item.origin.why.length > 0,
)

const count = computed(
  () => trail.value.length + props.item.relates.length + props.item.files.length + (hasOrigin.value ? 1 : 0),
)

function stamp(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  const p = (n: number): string => (n < 10 ? `0${n}` : String(n))
  return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`
}
</script>

<template>
  <div class="ctx">
    <button type="button" class="ctx-toggle" :aria-expanded="open" @click.stop="open = !open">
      <ChevronRight :size="12" :stroke-width="2.5" :class="['ctx-caret', { down: open }]" />
      脉络（{{ count }}）
    </button>

    <div v-if="open" class="ctx-body" @click.stop>
      <template v-if="item.detail.trim().length > 0">
        <p class="ctx-label">正文</p>
        <p class="ctx-text xd-select"><DetailText :text="item.detail" /></p>
      </template>

      <template v-if="hasOrigin">
        <p class="ctx-label">来源</p>
        <p v-if="item.origin.from" class="ctx-text xd-select">{{ item.origin.from }}</p>
        <p v-if="item.origin.why" class="ctx-text dim xd-select">{{ item.origin.why }}</p>
      </template>

      <template v-if="item.relates.length > 0">
        <p class="ctx-label">关联待办</p>
        <div class="ctx-chips">
          <IdChip v-for="rid in item.relates" :key="rid" :id="rid" @copied="emit('result', '已复制编号')" />
        </div>
      </template>

      <template v-if="item.files.length > 0">
        <p class="ctx-label">关联文件</p>
        <div class="ctx-chips">
          <ArtifactChip
            v-for="f in item.files"
            :key="f"
            :path="f"
            :lib-root="libRoot"
            @result="emit('result', $event)"
          />
        </div>
      </template>

      <template v-if="trail.length > 0">
        <p class="ctx-label">流水</p>
        <ul class="ctx-trail">
          <li v-for="(t, i) in trail" :key="`${t.at}-${i}`">
            <span class="ctx-time">{{ stamp(t.at) }}</span>
            <span class="ctx-text xd-select">{{ t.text }}</span>
          </li>
        </ul>
      </template>
    </div>
  </div>
</template>

<style scoped>
.ctx {
  margin-top: 6px;
}

.ctx-toggle {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 0;
  border: none;
  background: none;
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  transition: color 0.15s var(--xd-ease);
}

.ctx-toggle:hover {
  color: var(--xd-accent);
}

.ctx-caret {
  transition: transform 0.16s var(--xd-ease);
}

.ctx-caret.down {
  transform: rotate(90deg);
}

.ctx-body {
  margin-top: 6px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--xd-card-sub);
  border: 1px solid var(--xd-border-soft);
}

.ctx-label {
  margin: 8px 0 3px;
  color: var(--xd-text-dim);
  font-size: calc(12px * var(--xd-font-scale));
  letter-spacing: 0.3px;
}

.ctx-label:first-child {
  margin-top: 0;
}

.ctx-text {
  margin: 0;
  color: var(--xd-text-sub);
  font-size: calc(13.8px * var(--xd-font-scale));
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.ctx-text.dim {
  color: var(--xd-text-dim);
}

.ctx-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.ctx-trail {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.ctx-trail li {
  display: flex;
  gap: 6px;
  align-items: baseline;
}

.ctx-time {
  flex: none;
  color: var(--xd-text-dim);
  font-size: calc(12px * var(--xd-font-scale));
  font-family: 'Cascadia Code', Consolas, monospace;
}
</style>
