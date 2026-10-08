<script setup lang="ts">
// 审批卡：小刀交了活、等老板拍板（status = waiting 且在小刀视图）。
//
// 正文只放**概要 + 产出文件**这两样 —— 拍板要看的是"交了什么、结论是什么"，
// 底稿折进脉络里，别在这里堆成长文。
//
// 三个出口点开就地变留言框，不跳页不弹窗：点「退回」本意常常是"先停下"，
// 所以三个出口必须分开 —— 退回是让小刀重做，挂起是让它别再动。
import { computed, ref } from 'vue'
import { ArrowLeftRight, Check, Pencil, Trash2 } from 'lucide-vue-next'
import IdChip from './IdChip.vue'
import ArtifactChip from './ArtifactChip.vue'
import ContextBlock from './ContextBlock.vue'
import DetailText from './DetailText.vue'
import RemoveConfirm from './RemoveConfirm.vue'
import { briefOf, artifactPaths } from '../core/brief'
import { useDomains } from '../composables/useDomains'

const { label: domainName } = useDomains()
import { isOverdue } from '../core/view'
import type { ReviewAction, TodoItem } from '../core/types'

const props = defineProps<{ item: TodoItem; libRoot: string }>()

const emit = defineEmits<{
  (e: 'review', action: ReviewAction, comment: string): void
  (e: 'done'): void
  (e: 'remove'): void
  (e: 'edit'): void
  (e: 'transfer'): void
  (e: 'note', text: string): void
  (e: 'result', text: string): void
}>()

type Pending = ReviewAction | null

const pending = ref<Pending>(null)
const comment = ref('')
const confirming = ref(false)

const brief = computed(() => briefOf(props.item))
const files = computed(() => artifactPaths(props.item))
const overdue = computed(() => isOverdue(props.item))

/** 上一条非"通过"的留言 —— 小刀要知道上次为什么被退回来 */
const lastNote = computed(() => {
  const r = [...props.item.reviews].reverse().find((x) => x.action !== 'approved')
  if (!r) return null
  const label = r.action === 'rejected' ? '上次退回' : '上次挂起'
  return r.comment.length > 0 ? `${label}：${r.comment}` : null
})

const placeholders: Record<ReviewAction, string> = {
  rejected: '哪里不行？（可空，写了小刀照着改）',
  shelved: '为什么先放着？（写下来，将来恢复时看得到）',
  approved: '要补充什么？（可空）',
}

const submitLabels: Record<ReviewAction, string> = {
  rejected: '退回给小刀',
  shelved: '挂起这条',
  approved: '通过',
}

function pick(action: ReviewAction): void {
  pending.value = action
  comment.value = ''
}

function submit(): void {
  if (pending.value === null) return
  emit('review', pending.value, comment.value.trim())
  pending.value = null
  comment.value = ''
}

function cancel(): void {
  pending.value = null
  comment.value = ''
}
</script>

<template>
  <div class="card review">
    <div class="card-head">
      <IdChip :id="props.item.id" @copied="emit('result', '已复制编号')" />
      <span class="domain">{{ domainName(props.item.domain) }}</span>
      <span class="badge waiting">等待拍板</span>
      <span v-if="overdue" class="overdue">已过期</span>

      <span class="spacer" />

      <div class="ops">
        <button type="button" class="op" title="转给我自己" @click.stop="emit('transfer')">
          <ArrowLeftRight :size="14" :stroke-width="2" />
        </button>
        <button type="button" class="op ok" title="完成" @click.stop="emit('done')">
          <Check :size="15" :stroke-width="2.4" />
        </button>
        <button type="button" class="op" title="修改" @click.stop="emit('edit')">
          <Pencil :size="13" :stroke-width="2" />
        </button>
        <button type="button" class="op danger" title="移出列表" @click.stop="confirming = true">
          <Trash2 :size="13" :stroke-width="2" />
        </button>
      </div>
    </div>

    <div class="card-title xd-select">{{ props.item.title }}</div>

    <p v-if="brief.length > 0" class="card-brief xd-select"><DetailText :text="brief" /></p>

    <div v-if="files.length > 0" class="card-files">
      <span class="files-label">产出</span>
      <ArtifactChip
        v-for="f in files"
        :key="f"
        :path="f"
        :lib-root="props.libRoot"
        @result="emit('result', $event)"
      />
    </div>

    <p v-if="lastNote" class="last-note">{{ lastNote }}</p>

    <!-- 三个出口：独占一行、等宽 -->
    <div v-if="pending === null" class="exits">
      <button type="button" class="exit ok" @click.stop="pick('approved')">通过</button>
      <button type="button" class="exit back" @click.stop="pick('rejected')">退回</button>
      <button type="button" class="exit hold" @click.stop="pick('shelved')">先挂起</button>
    </div>

    <div v-else class="reply" @click.stop>
      <input
        v-model="comment"
        class="reply-input"
        type="text"
        :placeholder="placeholders[pending]"
        spellcheck="false"
        @keydown.enter="submit"
        @keydown.esc="cancel"
      />
      <div class="reply-actions">
        <button type="button" class="reply-btn ghost" @click="cancel">取消</button>
        <button type="button" class="reply-btn primary" @click="submit">
          {{ submitLabels[pending] }}
        </button>
      </div>
    </div>

    <ContextBlock
      :item="props.item"
      :lib-root="props.libRoot"
      @note="emit('note', $event)"
      @result="emit('result', $event)"
    />

    <RemoveConfirm
      v-if="confirming"
      :title="props.item.title"
      @confirm="
        confirming = false;
        emit('remove')
      "
      @cancel="confirming = false"
    />
  </div>
</template>

<style scoped>
.card {
  position: relative;
  padding: 10px 12px 10px 14px;
  border-radius: var(--xd-radius);
  background: var(--xd-card);
  border: 1px solid color-mix(in srgb, var(--xd-orange) 42%, var(--xd-border));
  border-left: 3px solid var(--xd-orange);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.04);
  transition: box-shadow 0.18s var(--xd-ease), transform 0.18s var(--xd-ease);
}

.card:hover {
  box-shadow: var(--xd-shadow-card);
  transform: translateY(-1px);
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.spacer {
  flex: 1;
}

.domain {
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(13.2px * var(--xd-font-scale));
}

.badge.waiting {
  padding: 1px 7px;
  border-radius: 999px;
  font-size: calc(13.2px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-orange);
  background: color-mix(in srgb, var(--xd-orange) 16%, transparent);
}

.overdue {
  padding: 1px 6px;
  border-radius: 999px;
  font-size: calc(12.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-red);
  background: color-mix(in srgb, var(--xd-red) 14%, transparent);
}

.ops {
  display: flex;
  gap: 1px;
  opacity: 0.55;
  transition: opacity 0.16s var(--xd-ease);
}

.card:hover .ops,
.card:focus-within .ops {
  opacity: 1;
}

.op {
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

.op:hover {
  background: var(--xd-card-sub);
  color: var(--xd-text);
}

.op.ok:hover {
  color: var(--xd-green);
  background: color-mix(in srgb, var(--xd-green) 12%, transparent);
}

.op.danger:hover {
  color: var(--xd-red);
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
}

.card-title {
  margin: 6px 0 0;
  font-size: calc(16.2px * var(--xd-font-scale));
  font-weight: 600;
  line-height: 1.45;
  color: var(--xd-text);
  word-break: break-word;
}

.card-brief {
  margin: 4px 0 0;
  font-size: calc(14.4px * var(--xd-font-scale));
  line-height: 1.6;
  color: var(--xd-text-sub);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.card-files {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 7px;
  flex-wrap: wrap;
}

.files-label {
  color: var(--xd-text-dim);
  font-size: calc(12.6px * var(--xd-font-scale));
}

.last-note {
  margin: 7px 0 0;
  padding: 5px 8px;
  border-radius: 7px;
  background: var(--xd-card-sub);
  border: 1px solid var(--xd-border-soft);
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: 1.5;
}

.exits {
  display: flex;
  gap: 6px;
  margin-top: 9px;
}

.exit {
  flex: 1;
  padding: 6px 0;
  border-radius: 8px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  font-weight: 600;
  transition: all 0.15s var(--xd-ease);
}

.exit.ok:hover {
  border-color: var(--xd-green);
  background: color-mix(in srgb, var(--xd-green) 13%, transparent);
  color: var(--xd-green);
}

.exit.back:hover {
  border-color: var(--xd-red);
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
  color: var(--xd-red);
}

.exit.hold:hover {
  border-color: var(--xd-text-dim);
  background: var(--xd-border-soft);
  color: var(--xd-text);
}

.reply {
  margin-top: 9px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 9px 10px;
  border-radius: 10px;
  background: var(--xd-accent-soft);
  border: 1px solid color-mix(in srgb, var(--xd-accent) 28%, transparent);
}

.reply-input {
  width: 100%;
  padding: 6px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 7px;
  background: var(--xd-card);
  outline: none;
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: border-color 0.15s var(--xd-ease);
}

.reply-input:focus {
  border-color: var(--xd-accent);
}

.reply-actions {
  display: flex;
  gap: 8px;
}

.reply-btn {
  flex: 1;
  padding: 5px 10px;
  border-radius: 7px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.reply-btn.ghost:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.reply-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.reply-btn.primary:hover {
  background: var(--xd-accent-hover);
}
</style>
