<script setup lang="ts">
// 转交代办。
//
// 两个方向的待遇不同：转给小刀**必须填交代**（没交代就转过去等于让它猜），
// 转给我自己不需要交代（但仍会落一行时间戳留痕）。
import { computed, ref } from 'vue'
import { OWNER_AGENT, OWNER_LABELS } from '../core/constants'
import type { Owner } from '../core/types'

const props = defineProps<{ owner: Owner; title: string }>()
const emit = defineEmits<{
  (e: 'submit', to: Owner, note: string): void
  (e: 'cancel'): void
}>()

const to = computed<Owner>(() => (props.owner === OWNER_AGENT ? 'user' : OWNER_AGENT))
const note = ref('')

const placeholder = computed(() =>
  to.value === OWNER_AGENT
    ? '要小刀做什么？（必填，会写进这条的底稿里）'
    : '自己接手安排（可空）',
)

const canSubmit = computed(() => to.value !== OWNER_AGENT || note.value.trim().length > 0)

function submit(): void {
  if (!canSubmit.value) return
  emit('submit', to.value, note.value.trim())
}
</script>

<template>
  <div class="tf" @click.stop>
    <p class="tf-head">
      转给 <strong>{{ OWNER_LABELS[to] }}</strong>
      <span class="tf-title">{{ props.title }}</span>
    </p>
    <input
      v-model="note"
      class="tf-input"
      type="text"
      :placeholder="placeholder"
      spellcheck="false"
      @keydown.enter="submit"
      @keydown.esc="emit('cancel')"
    />
    <div class="tf-actions">
      <button type="button" class="tf-btn ghost" @click="emit('cancel')">取消</button>
      <button type="button" class="tf-btn primary" :disabled="!canSubmit" @click="submit">
        {{ to === OWNER_AGENT ? '交给小刀' : '转给我' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.tf {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 11px;
  border-radius: 10px;
  background: var(--xd-accent-soft);
  border: 1px solid color-mix(in srgb, var(--xd-accent) 30%, transparent);
}

.tf-head {
  margin: 0;
  font-size: calc(14.4px * var(--xd-font-scale));
  color: var(--xd-text);
}

.tf-title {
  margin-left: 6px;
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
}

.tf-input {
  width: 100%;
  padding: 6px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 7px;
  background: var(--xd-card);
  outline: none;
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: border-color 0.15s var(--xd-ease);
}

.tf-input:focus {
  border-color: var(--xd-accent);
}

.tf-actions {
  display: flex;
  gap: 8px;
}

.tf-btn {
  flex: 1;
  padding: 5px 10px;
  border-radius: 7px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.tf-btn.ghost:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.tf-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.tf-btn.primary:hover:not(:disabled) {
  background: var(--xd-accent-hover);
}

.tf-btn.primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
