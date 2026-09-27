<script setup lang="ts">
// 就地编辑：点 ✎ 之后卡片本身变成表单，不跳页、不弹窗。
// 保存走 applyPatch —— 改了什么会自动记一条流水。
import { ref } from 'vue'
import DraftFieldsEditor from './DraftFieldsEditor.vue'
import { draftFromItem, type DraftFields } from '../core/draft'
import type { TodoItem } from '../core/types'

const props = defineProps<{ item: TodoItem }>()
const emit = defineEmits<{
  (e: 'save', draft: DraftFields): void
  (e: 'cancel'): void
}>()

const draft = ref<DraftFields>(draftFromItem(props.item))

function save(): void {
  if (draft.value.title.trim().length === 0) return
  emit('save', { ...draft.value, title: draft.value.title.trim() })
}
</script>

<template>
  <div class="edit" @click.stop>
    <input
      v-model="draft.title"
      class="edit-title"
      type="text"
      placeholder="标题"
      spellcheck="false"
      @keydown.enter="save"
      @keydown.esc="emit('cancel')"
    />

    <textarea
      v-model="draft.detail"
      class="edit-area"
      rows="3"
      placeholder="底稿（只追加，别删旧的）"
      @keydown.esc="emit('cancel')"
    />

    <input
      v-model="draft.summary"
      class="edit-summary"
      type="text"
      placeholder="汇报概要（一句话结论；空着就自动取底稿第一段）"
      spellcheck="false"
      @keydown.enter="save"
      @keydown.esc="emit('cancel')"
    />

    <DraftFieldsEditor v-model="draft" />

    <div class="edit-actions">
      <button type="button" class="edit-btn ghost" @click="emit('cancel')">取消</button>
      <button
        type="button"
        class="edit-btn primary"
        :disabled="draft.title.trim().length === 0"
        @click="save"
      >
        保存
      </button>
    </div>
  </div>
</template>

<style scoped>
.edit {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 11px 12px;
  border-radius: var(--xd-radius);
  background: var(--xd-card);
  border: 1px solid color-mix(in srgb, var(--xd-accent) 40%, transparent);
  box-shadow: var(--xd-shadow-card);
}

.edit-title,
.edit-summary,
.edit-area {
  width: 100%;
  padding: 6px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 7px;
  background: var(--xd-card-sub);
  color: var(--xd-text);
  outline: none;
  font-size: calc(15px * var(--xd-font-scale));
  transition: border-color 0.15s var(--xd-ease);
}

.edit-title {
  font-size: calc(16.2px * var(--xd-font-scale));
  font-weight: 600;
}

.edit-area {
  line-height: 1.6;
  resize: vertical;
}

.edit-title:focus,
.edit-summary:focus,
.edit-area:focus {
  border-color: var(--xd-accent);
}

.edit-actions {
  display: flex;
  gap: 8px;
}

.edit-btn {
  flex: 1;
  padding: 6px 10px;
  border-radius: 7px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.edit-btn.ghost:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.edit-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.edit-btn.primary:hover:not(:disabled) {
  background: var(--xd-accent-hover);
}

.edit-btn.primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
