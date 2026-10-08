<script setup lang="ts">
// 脉络：正文 / 来源 / 关联待办 / 关联文件 / 流水。
//
// 卡片上只留一句概要，其余全折进这里 —— 点开才展开，避免一张卡占满半个屏幕。
import { computed, ref } from 'vue'
import { ChevronRight } from 'lucide-vue-next'
import DetailText from './DetailText.vue'
import IdChip from './IdChip.vue'
import ArtifactChip from './ArtifactChip.vue'
import { TRAIL_KIND_LABELS } from '../core/constants'
import { formatStamp, trailNewestFirst } from '../core/view'
import type { TodoItem } from '../core/types'

const props = defineProps<{ item: TodoItem; libRoot: string }>()
const emit = defineEmits<{
  (e: 'result', text: string): void
  (e: 'note', text: string): void
}>()

const open = ref(false)
const noteText = ref('')

/**
 * 手动往流水里记一笔。
 *
 * 为什么要有这个口子：流水原先只能由动作自动产生（改状态、编辑、评审、移交），
 * 想自己跟进进度的人没地方写，只能往正文里堆 —— 可正文说的是「这条要干什么」，
 * 进度是「后来发生了什么」，混在一起正文越攒越长，而且正文自己不带时间。
 */
function submitNote(): void {
  const text = noteText.value.trim()
  if (text.length === 0) return
  emit('note', text)
  noteText.value = ''
}

// 从新到旧。**按时间排，不是把数组倒过来** —— 文件里的先后取决于谁写的
// （桌面端往后追加，AI 侧的脚本未必），倒过来用等于赌它存的顺序
const trail = computed(() => trailNewestFirst(props.item.trail))
const hasOrigin = computed(
  () => props.item.origin.from.length > 0 || props.item.origin.why.length > 0,
)

const count = computed(
  () => trail.value.length + props.item.relates.length + props.item.files.length + (hasOrigin.value ? 1 : 0),
)

/** 跨年补年份那套口径在 `view.formatStamp` 里，跟卡片上的期限共用一个 */
function stamp(iso: string): string {
  return formatStamp(iso)
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

      <!-- 「流水」这块不随有没有流水而隐藏：正是一条还没有流水的待办，
           才最需要这个输入框 —— 否则想跟进进度的人压根找不到地方写 -->
      <p class="ctx-label">流水</p>
      <input
        v-model="noteText"
        class="ctx-note"
        type="text"
        placeholder="记一笔（回车存进流水，自动带时间）"
        spellcheck="false"
        @keydown.enter="submitNote"
        @keydown.esc="noteText = ''"
      />
      <ul v-if="trail.length > 0" class="ctx-trail">
        <li v-for="(t, i) in trail" :key="`${t.at}-${i}`">
          <span class="ctx-time">
            {{ stamp(t.at) }}
            <span class="ctx-kind" :class="`k-${t.kind}`">{{ TRAIL_KIND_LABELS[t.kind] ?? '' }}</span>
          </span>
          <span class="ctx-text xd-select">{{ t.text }}</span>
        </li>
      </ul>
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

/* 记一笔：展开脉络就能写，回车直接进流水（自动带时间）。
   手动跟进进度以前只能往正文里塞 —— 正文越攒越长，还不带时间 */
.ctx-note {
  width: 100%;
  margin-bottom: 7px;
  padding: 5px 8px;
  border: 1px solid var(--xd-border);
  border-radius: 6px;
  background: var(--xd-card-sub);
  color: var(--xd-text);
  outline: none;
  font-size: calc(13.2px * var(--xd-font-scale));
  transition: border-color 0.15s var(--xd-ease);
}

.ctx-note:focus {
  border-color: var(--xd-accent);
}

.ctx-note::placeholder {
  color: var(--xd-text-dim);
}

.ctx-trail {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  /* 条与条之间留足空隙：一行时间 + 一段话算一条，挤在一起分不清哪段归哪个时间 */
  gap: 7px;
}

/* 时间独占一行，下一行才是这条流水说了什么 ——
   原来时间是贴在这段话左边的，文字一长就被时间挤成窄条，读起来要来回折眼 */
.ctx-trail li {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
}

.ctx-time {
  flex: none;
  color: var(--xd-text-dim);
  font-size: calc(12px * var(--xd-font-scale));
  font-family: 'Cascadia Code', Consolas, monospace;
}

/* 类型标：评审不单独开一栏，就靠这个标在流水里挑出来 */
.ctx-kind {
  display: inline-block;
  margin-left: 5px;
  padding: 0 5px;
  border: 1px solid var(--xd-border-soft);
  border-radius: 4px;
  color: var(--xd-text-dim);
  font-family: inherit;
  font-size: calc(11px * var(--xd-font-scale));
  line-height: calc(16px * var(--xd-font-scale));
}

/* 评审和转手是"别人动了这条"，标出来 —— 翻一长串流水时先看见这两类 */
.ctx-kind.k-review,
.ctx-kind.k-transfer {
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}
</style>
