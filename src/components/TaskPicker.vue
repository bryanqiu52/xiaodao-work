<script setup lang="ts">
// 待办选择浮层：从现有清单里挑一条（番茄钟绑定）或挑几条（关联待办）。
//
// 为什么要有它、而不是让人手打编号：`relates` 存的是 id，手打一个长 id
// （`imtwmhii7-b1c5lf`）几乎必错，错了还不报错 —— 查不到就静默跳过。
// 从列表里点，就没有填错的余地。
//
// 浮层**必须 Teleport 到 body**（项目硬约定，没有例外）：卡片 hover 有 `transform`，
// 祖先一旦有 transform，后代的 `position: fixed` 就不再相对视口定位，会跟着卡片跑。
import { computed, ref, watch } from 'vue'
import { Check, Search, X } from 'lucide-vue-next'
import { OWNER_LABELS, PRIORITY_LABELS, STATUS_LABELS } from '../core/constants'
import { compareWithinGroup, matchesKeyword, shortId, todayLocal } from '../core/view'
import type { TodoItem } from '../core/types'
import { todoStore } from '../stores/todo'

const props = withDefaults(
  defineProps<{
    open: boolean
    title?: string
    /** 多选（关联待办用）；单选点一下就选定并关闭 */
    multiple?: boolean
    /** 已经在列表里的，不再重复列出 */
    excludeIds?: string[]
  }>(),
  { title: '选一条待办', multiple: false, excludeIds: () => [] },
)

const emit = defineEmits<{
  (e: 'pick', ids: string[]): void
  (e: 'close'): void
}>()

const keyword = ref('')
const selected = ref<string[]>([])

/**
 * 中文输入法：**拼字期间不要拿半成品去搜**。
 * 输入过程中 `input` 事件会把拼音字母（"zhang"）也喂进来，搜出来全是莫名其妙的东西。
 * 等 `compositionend`（上屏）再更新关键词 —— 面板搜索框那边踩过同一个坑。
 */
const composing = ref(false)

function onSearchInput(e: Event): void {
  if (composing.value) return
  keyword.value = (e.target as HTMLInputElement).value
}

function onCompositionEnd(e: Event): void {
  composing.value = false
  keyword.value = (e.target as HTMLInputElement).value
}

/**
 * 候选：未删除、未完成、不在排除名单里的。
 *
 * **已完成的不列**：这两个场景（挑一条现在要专注的、关联一条还在办的）
 * 都不会要一条已经做完的。要找已完成的，那是「已完成」清单的活。
 */
const candidates = computed<TodoItem[]>(() => {
  const excluded = new Set(props.excludeIds)
  const kw = keyword.value.trim()
  return todoStore.items
    .filter((i) => i.deletedAt === null && i.doneAt === null && !excluded.has(i.id))
    .filter((i) => matchesKeyword(i, kw))
    .sort(compareWithinGroup)
})

const pendingCount = computed(
  () => todoStore.items.filter((i) => i.deletedAt === null && i.doneAt === null).length,
)

/** 今天到期 / 已过期的标出来 —— 挑的时候这个信息最有用 */
function urgencyOf(item: TodoItem): string {
  const day = item.dueAt === null ? '' : item.dueAt.slice(0, 10)
  if (day === '') return ''
  const today = todayLocal()
  if (day < today) return '过期'
  if (day === today) return '今天'
  return ''
}

// 每次打开都从干净状态开始：上回选了一半就关掉的残留，会让人以为已经选好了
watch(
  () => props.open,
  (open) => {
    if (open) {
      keyword.value = ''
      selected.value = []
    }
  },
)

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

function choose(item: TodoItem): void {
  if (!props.multiple) {
    emit('pick', [item.id])
    return
  }
  const at = selected.value.indexOf(item.id)
  if (at >= 0) selected.value.splice(at, 1)
  else selected.value.push(item.id)
}

function confirmMulti(): void {
  if (selected.value.length === 0) return
  emit('pick', [...selected.value])
}
</script>

<template>
  <Teleport to="body">
    <template v-if="props.open">
      <div class="tp-scrim" @click="emit('close')" />
      <div
        class="tp-sheet"
        role="dialog"
        :aria-label="props.title"
        tabindex="-1"
        @click.stop
        @keydown="onKey"
      >
        <div class="tp-head">
          <span class="tp-title">{{ props.title }}</span>
          <button type="button" class="tp-x" aria-label="关闭" @click="emit('close')">
            <X :size="14" :stroke-width="2" />
          </button>
        </div>

        <div class="tp-search">
          <Search :size="13" :stroke-width="2" class="tp-search-icon" />
          <input
            :value="keyword"
            class="tp-search-input"
            type="text"
            placeholder="搜标题 / 编号"
            spellcheck="false"
            @input="onSearchInput"
            @compositionstart="composing = true"
            @compositionend="onCompositionEnd"
            @keydown.esc="keyword = ''"
          />
          <button v-if="keyword" type="button" class="tp-clear" aria-label="清空搜索" @click="keyword = ''">
            <X :size="11" :stroke-width="2.4" />
          </button>
        </div>

        <div class="tp-list">
          <button
            v-for="it in candidates"
            :key="it.id"
            type="button"
            class="tp-row"
            :class="{ on: props.multiple && selected.includes(it.id) }"
            :title="it.title"
            @click="choose(it)"
          >
            <span v-if="props.multiple" class="tp-box" :class="{ on: selected.includes(it.id) }">
              <Check v-if="selected.includes(it.id)" :size="11" :stroke-width="3" />
            </span>
            <span class="tp-id">{{ shortId(it.id) }}</span>
            <span class="tp-name">{{ it.title }}</span>
            <span class="tp-badges">
              <span v-if="urgencyOf(it)" class="tp-badge bad">{{ urgencyOf(it) }}</span>
              <span class="tp-badge">{{ OWNER_LABELS[it.owner] ?? it.owner }}</span>
              <span class="tp-badge">{{ STATUS_LABELS[it.status] ?? it.status }}</span>
              <span class="tp-badge w">{{ PRIORITY_LABELS[it.priority] ?? it.priority }}</span>
            </span>
          </button>

          <div v-if="candidates.length === 0" class="tp-empty">
            <p class="tp-empty-mark">{{ keyword.trim() ? '🔍' : '📋' }}</p>
            <p class="tp-empty-title">
              {{ keyword.trim() ? '没有匹配的待办' : '没有可挑的待办' }}
            </p>
            <p class="tp-empty-desc">
              {{
                keyword.trim()
                  ? '换个词试试，或者先清空搜索'
                  : pendingCount === 0
                    ? '清单里还没有未完成的事'
                    : '剩下的都在排除名单里了'
              }}
            </p>
          </div>
        </div>

        <div class="tp-actions">
          <button type="button" class="tp-btn ghost" @click="emit('close')">取消</button>
          <button
            v-if="props.multiple"
            type="button"
            class="tp-btn primary"
            :disabled="selected.length === 0"
            @click="confirmMulti"
          >
            选好了（{{ selected.length }}）
          </button>
        </div>
      </div>
    </template>
  </Teleport>
</template>

<style scoped>
/* 遮罩跟窗口四角同样圆角：窗口是透明的，不给圆角的话它会像补丁一样糊在窗外那四个角上 */
.tp-scrim {
  position: fixed;
  inset: 0;
  z-index: 75;
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.35);
}

.tp-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  translate: -50% -50%;
  z-index: 80;
  display: flex;
  flex-direction: column;
  width: min(460px, calc(100vw - 28px));
  max-height: calc(100vh - 60px);
  padding: 13px 14px 11px;
  border-radius: var(--xd-radius-lg);
  /* 半透明是刻意的：磨砂糊的就是透出来的那部分，底色一实就白加了 */
  background: var(--xd-sheet-bg);
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
  animation: tp-in 0.18s var(--xd-ease);
}

@keyframes tp-in {
  from {
    opacity: 0;
    scale: 0.96;
  }
}

.tp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.tp-title {
  font-size: calc(15.6px * var(--xd-font-scale));
  font-weight: 600;
}

.tp-x {
  flex: none;
  display: grid;
  place-items: center;
  width: calc(22px * var(--xd-font-scale));
  height: calc(22px * var(--xd-font-scale));
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--xd-text-dim);
}

.tp-x:hover {
  background: var(--xd-card-sub);
  color: var(--xd-text);
}

.tp-search {
  flex: none;
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 0 9px;
  border-radius: 999px;
  background: var(--xd-card);
  border: 1px solid var(--xd-border);
  transition: border-color 0.15s var(--xd-ease), box-shadow 0.15s var(--xd-ease);
}

.tp-search:focus-within {
  border-color: var(--xd-accent);
  box-shadow: 0 0 0 3px var(--xd-accent-soft);
}

.tp-search-icon {
  flex: none;
  color: var(--xd-text-dim);
}

.tp-search-input {
  flex: 1;
  min-width: 0;
  padding: 6px 0;
  border: none;
  background: transparent;
  outline: none;
  color: var(--xd-text);
  font-size: calc(14.4px * var(--xd-font-scale));
}

.tp-clear {
  flex: none;
  display: grid;
  place-items: center;
  width: calc(16px * var(--xd-font-scale));
  height: calc(16px * var(--xd-font-scale));
  border: none;
  border-radius: 50%;
  background: var(--xd-border-soft);
  color: var(--xd-text-dim);
}

.tp-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin: 9px 0 0;
  padding-right: 2px;
}

.tp-row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 7px 8px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--xd-text);
  text-align: left;
  transition: background 0.14s var(--xd-ease), border-color 0.14s var(--xd-ease);
}

.tp-row:hover {
  background: var(--xd-card-sub);
}

.tp-row.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
}

.tp-box {
  flex: none;
  display: grid;
  place-items: center;
  width: calc(15px * var(--xd-font-scale));
  height: calc(15px * var(--xd-font-scale));
  border: 1px solid var(--xd-border);
  border-radius: 4px;
  color: var(--xd-accent-text);
}

.tp-box.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
}

.tp-id {
  flex: none;
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.tp-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: calc(14.4px * var(--xd-font-scale));
}

.tp-badges {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.tp-badge {
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--xd-card-sub);
  color: var(--xd-text-dim);
  font-size: calc(11.6px * var(--xd-font-scale));
  line-height: calc(17px * var(--xd-font-scale));
}

.tp-badge.bad {
  background: color-mix(in srgb, var(--xd-red) 14%, transparent);
  color: var(--xd-red);
}

.tp-badge.w {
  color: var(--xd-text-sub);
}

.tp-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 22px 12px;
  text-align: center;
}

.tp-empty-mark {
  margin: 0;
  font-size: calc(24px * var(--xd-font-scale));
  opacity: 0.7;
}

.tp-empty-title {
  margin: 0;
  font-size: calc(14.4px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text-sub);
}

.tp-empty-desc {
  margin: 0;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.tp-actions {
  flex: none;
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.tp-btn {
  flex: 1;
  padding: 6px 10px;
  border-radius: 8px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.tp-btn.ghost:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.tp-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.tp-btn.primary:hover:not(:disabled) {
  background: var(--xd-accent-hover);
}

.tp-btn.primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 窄窗口下徽章让位给标题 —— 360px 宽时全挤在一行会把标题压到只剩一个点 */
@media (max-width: 420px) {
  .tp-badges {
    display: none;
  }
}
</style>
