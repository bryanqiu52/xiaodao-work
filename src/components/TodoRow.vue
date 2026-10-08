<script setup lang="ts">
// 普通卡片。
//
// 左侧 3px 色条**只说权重**（红高 / 蓝中 / 灰低）—— 不看文字先知道轻重。
// 操作图标常驻但低对比，hover 才提亮：既不用先点开菜单，也不抢标题的注意力。
import { computed, ref } from 'vue'
import { ArrowLeftRight, Check, Clock, Pencil, Trash2 } from 'lucide-vue-next'
import IdChip from './IdChip.vue'
import ArtifactChip from './ArtifactChip.vue'
import ContextBlock from './ContextBlock.vue'
import StatusBar from './StatusBar.vue'
import DetailText from './DetailText.vue'
import RemoveConfirm from './RemoveConfirm.vue'
import { briefOf, artifactPaths } from '../core/brief'
import { DEFAULT_STATUS, normalizeSort, PRIORITY_LABELS, STATUS_LABELS } from '../core/constants'
import { useDomains } from '../composables/useDomains'
import { configStore } from '../stores/config'

const { label: domainName } = useDomains()
import { formatStamp, isDueToday, isOverdue, lastTouchedAt } from '../core/view'
import type { Status, TodoItem } from '../core/types'

const props = defineProps<{
  item: TodoItem
  agentView: boolean
  libRoot: string
  /**
   * 显示归属标签（我的 / 小刀的）。
   * 正常列表里归属由顶部 tab 表明，不需要每张卡片都写一遍；
   * 只有**全局搜索**才需要 —— 它跨归属出结果，不标出来会让人以为这是自己的事。
   */
  showOwner?: boolean
}>()

const emit = defineEmits<{
  (e: 'done'): void
  (e: 'remove'): void
  (e: 'edit'): void
  (e: 'transfer'): void
  (e: 'snooze'): void
  (e: 'status', next: Status): void
  (e: 'note', text: string): void
  (e: 'result', text: string): void
}>()

const confirming = ref(false)

const brief = computed(() => briefOf(props.item))
const files = computed(() => artifactPaths(props.item))
const overdue = computed(() => isOverdue(props.item))
/**
 * 要不要出现「稍后再说」。
 *
 * **只在已过期或今天到期时出现** —— 提醒就是在这两类上触发的（见 `core/remind.ts`），
 * 别的场合给个"稍后"按钮没有意义（还没到日子，往后推什么呢）。
 * 放在这里而不是放进通知里：Windows 通知的按钮要跨平台支持，不可靠；
 * 而且用户看到通知后本来就会回来点这条待办，这个位置更顺手。
 */
const needsNudge = computed(() => overdue.value || isDueToday(props.item))

/** 跨年补上年份：不然明年的「03-05」跟今年的长得一模一样，看不出是哪一年 */
function dueText(): string {
  const d = props.item.dueAt
  if (!d) return ''
  return formatStamp(d, false)
}

/**
 * 「最近更新」模式下，卡片上要写出"它最近是什么时候动的"。
 *
 * 排序依据看不见是最难受的：顺序变了却说不清为什么，只会觉得"它怎么跑上去了"。
 * 只在按最近更新排时出现 —— 按重要程度排的时候这行是白噪音。
 */
const recentAt = computed(() =>
  normalizeSort(configStore.cfg.todo_sort) === 'recent' ? formatStamp(lastTouchedAt(props.item)) : '',
)
</script>

<template>
  <div class="card" :class="[`w-${props.item.priority}`, { 'is-deleted': props.item.deletedAt }]">
    <div class="card-head">
      <IdChip :id="props.item.id" @copied="emit('result', '已复制编号')" />
      <span class="domain">{{ domainName(props.item.domain) }}</span>
      <span v-if="props.showOwner" class="owner-tag">
        {{ props.item.owner === 'agent' ? '小刀的' : '我的' }}
      </span>
      <!-- 小刀视图常驻；「我的」这条只在**不是排队**时才冒出来。
           待办会在两边转手（转回给你时状态原样保留），一条「已暂停」的活回到你的列表
           却看不出它停着，那才是问题。常态的排队不标，免得满屏都是同一个词 -->
      <span
        v-if="props.agentView || props.item.status !== DEFAULT_STATUS"
        class="badge"
        :class="`st-${props.item.status}`"
      >
        {{ STATUS_LABELS[props.item.status] }}
      </span>
      <span v-if="props.item.deletedAt" class="deleted-tag">已删除</span>
      <span v-if="overdue" class="overdue">已过期</span>

      <span class="spacer" />

      <span v-if="props.item.dueAt" class="due" :class="{ late: overdue }">{{ dueText() }}</span>

      <div class="ops">
        <button
          v-if="needsNudge"
          type="button"
          class="op"
          title="稍后再说（推后 4 小时再提醒）"
          @click.stop="emit('snooze')"
        >
          <Clock :size="13" :stroke-width="2" />
        </button>
        <button type="button" class="op" title="转交" @click.stop="emit('transfer')">
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

    <div class="card-title xd-select">
      <a
        v-if="props.item.link"
        class="title-link"
        :href="props.item.link"
        target="_blank"
        rel="noreferrer"
        @click.stop
        >{{ props.item.title }}</a
      >
      <template v-else>{{ props.item.title }}</template>
    </div>

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

    <div v-if="props.agentView || recentAt.length > 0" class="card-meta">
      <span v-if="props.agentView" class="weight" :class="`w-${props.item.priority}`">
        权重 {{ PRIORITY_LABELS[props.item.priority] }}
      </span>
      <span v-if="recentAt.length > 0" class="recent-at">最近 {{ recentAt }}</span>
    </div>

    <ContextBlock
      :item="props.item"
      :lib-root="props.libRoot"
      @note="emit('note', $event)"
      @result="emit('result', $event)"
    />

    <div v-if="props.agentView" class="card-status">
      <StatusBar :status="props.item.status" @change="emit('status', $event)" />
    </div>

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
  border: 1px solid var(--xd-border);
  border-left: 3px solid var(--xd-w-mid);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.04);
  transition: box-shadow 0.18s var(--xd-ease), transform 0.18s var(--xd-ease),
    background 0.18s var(--xd-ease);
}

.card:hover {
  background: var(--xd-card-hover);
  box-shadow: var(--xd-shadow-card);
  transform: translateY(-1px);
}

.card.w-high {
  border-left-color: var(--xd-w-high);
}

.card.w-mid {
  border-left-color: var(--xd-w-mid);
}

.card.w-low {
  border-left-color: var(--xd-w-low);
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

.badge {
  padding: 1px 7px;
  border-radius: 999px;
  font-size: calc(13.2px * var(--xd-font-scale));
  font-weight: 600;
}

.badge.st-todo {
  color: var(--xd-blue);
  background: color-mix(in srgb, var(--xd-blue) 13%, transparent);
}

.badge.st-progress {
  color: var(--xd-accent);
  background: var(--xd-accent-soft);
}

.badge.st-waiting {
  color: var(--xd-orange);
  background: color-mix(in srgb, var(--xd-orange) 15%, transparent);
}

.badge.st-paused {
  color: var(--xd-text-dim);
  background: var(--xd-card-sub);
}

.badge.st-done {
  color: var(--xd-green);
  background: color-mix(in srgb, var(--xd-green) 13%, transparent);
}

/* 归属标签：只在全局搜索的结果里出现（见 showOwner 的说明） */
.owner-tag {
  padding: 1px 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--xd-text) 8%, transparent);
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.overdue {
  padding: 1px 6px;
  border-radius: 999px;
  font-size: calc(12.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-red);
  background: color-mix(in srgb, var(--xd-red) 14%, transparent);
}

/* 已删除：整张卡片压暗 + 一个标签。
   没有这两样的话，「显示已删除」开了也看不出任何区别 —— 已删条目跟正常的一模一样
   混在列表里，用户只会以为这个开关坏了（问过"这个设置项有用吗"）。 */
.card.is-deleted {
  opacity: 0.55;
}

.deleted-tag {
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px dashed var(--xd-border);
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.due {
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  font-family: 'Cascadia Code', Consolas, monospace;
}

.due.late {
  color: var(--xd-red);
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

.title-link {
  color: inherit;
  text-decoration: none;
  border-bottom: 1px solid color-mix(in srgb, var(--xd-accent) 50%, transparent);
}

.title-link:hover {
  color: var(--xd-accent);
}

.card-brief {
  margin: 4px 0 0;
  font-size: calc(14.4px * var(--xd-font-scale));
  line-height: 1.6;
  color: var(--xd-text-sub);
  /* 正文截三行：卡片里放太长会把列表撑成一坨 */
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

.card-meta {
  margin-top: 6px;
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 「最近更新」排序下那句"最近 10-08 14:22"：说明这条为什么排在这儿 */
.recent-at {
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  font-family: 'Cascadia Code', Consolas, monospace;
}

.weight {
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.weight.w-high {
  color: var(--xd-w-high);
}

.weight.w-mid {
  color: var(--xd-w-mid);
}

.card-status {
  margin-top: 8px;
}
</style>
