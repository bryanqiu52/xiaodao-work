<script setup lang="ts">
// 回顾：把清单里已经攒下的数据算成人看得懂的数字。
//
// 一页只回答四个问题：这段时间**干完了多少**、**还欠多少**、**时间花哪了**、
// **小刀那摊什么状况**。数字全部来自现有数据（`createdAt` / `doneAt` / `trail`
// 的时间戳 + 专注记录），不额外记任何"统计用的字段"。
//
// 口径上有两条硬规矩（改之前先读 `core/review.ts` 的注释）：
//   1. 软删除的条目一律不计入（`liveItems` 是所有对外输出的共同底数）；
//   2. 日期比较走 `YYYY-MM-DD` 字符串，不用 `new Date()` 比 —— 那个按 UTC 解析，会偏一天。
import { computed, onMounted, ref } from 'vue'
import { Timer } from 'lucide-vue-next'
import { STATUS_LABELS } from '../core/constants'
import { useDomains } from '../composables/useDomains'
import {
  agentBreakdown,
  domainStats,
  overviewOf,
  pendingByDomain,
  rangeOf,
  type ReviewRangeKind,
} from '../core/review'
import { fmtMinutes, focusCountIn, focusMinutesIn, loadFocusLog } from '../composables/useFocusLog'
import { todayLocal } from '../core/view'
import { todoStore } from '../stores/todo'

const { label: domainName } = useDomains()

const kind = ref<ReviewRangeKind>('week')

/**
 * 基准日只算一次，区间和页脚那句"按某天算"都用它。
 *
 * **不能在两处各调一次 `todayLocal()`**：跨零点那一瞬间两处会拿到不同的日期，
 * 出现"页脚说按 25 号算、数字却是按 26 号算的"这种对不上的事。
 */
const today = computed(() => todayLocal())
const range = computed(() => rangeOf(kind.value, today.value))

const overview = computed(() => overviewOf(todoStore.items, range.value))
const domains = computed(() => domainStats(todoStore.items, range.value))
const pending = computed(() => pendingByDomain(todoStore.items))
const agent = computed(() => agentBreakdown(todoStore.items))
const focusMinutes = computed(() => focusMinutesIn(range.value))
const focusCount = computed(() => focusCountIn(range.value))

/** 占比条按"相对最大值"归一化（照记账页那套）：首位顶满，次位一眼可比 */
const domainMax = computed(() => domains.value[0]?.done ?? 0)
function barWidth(done: number): number {
  if (domainMax.value <= 0) return 0
  return Math.max(4, Math.round((done / domainMax.value) * 100))
}

const pendingMax = computed(() => pending.value[0]?.count ?? 0)
function pendingWidth(count: number): number {
  if (pendingMax.value <= 0) return 0
  return Math.max(4, Math.round((count / pendingMax.value) * 100))
}

/** 空清单时给一句引导，而不是摆一排 0 */
const empty = computed(() => todoStore.items.filter((i) => i.deletedAt === null).length === 0)

onMounted(() => {
  // 幂等：App 启动时已读过就不重复读
  void loadFocusLog()
})
</script>

<template>
  <div class="rp">
    <div class="rp-head">
      <div class="rp-seg" role="group" aria-label="统计区间">
        <button
          type="button"
          class="rp-seg-btn"
          :class="{ on: kind === 'week' }"
          :aria-pressed="kind === 'week'"
          @click="kind = 'week'"
        >
          本周
        </button>
        <button
          type="button"
          class="rp-seg-btn"
          :class="{ on: kind === 'month' }"
          :aria-pressed="kind === 'month'"
          @click="kind = 'month'"
        >
          本月
        </button>
      </div>
      <p class="rp-span">{{ range.label }} · {{ range.span }}</p>
    </div>

    <div v-if="empty" class="rp-empty">
      <p class="rp-empty-mark">📊</p>
      <p class="rp-empty-title">还没有可回顾的数据</p>
      <p class="rp-empty-desc">先去「待办」记几条、干完几条 —— 这里就有了</p>
    </div>

    <template v-else>
      <!-- 四个数字卡：干完了 / 新来了 / 还欠着 / 平均耗时 -->
      <div class="rp-cards">
        <div class="rp-card">
          <span class="rp-num">{{ overview.doneCount }}</span>
          <span class="rp-label">完成</span>
          <span class="rp-sub">{{ range.label }}内</span>
        </div>
        <div class="rp-card">
          <span class="rp-num">{{ overview.createdCount }}</span>
          <span class="rp-label">新增</span>
          <span class="rp-sub">{{ range.label }}内记下的</span>
        </div>
        <div class="rp-card">
          <span class="rp-num" :class="{ warn: overview.overdueCount > 0 }">
            {{ overview.pendingCount }}
          </span>
          <span class="rp-label">还欠着</span>
          <span class="rp-sub">
            {{ overview.overdueCount > 0 ? `其中 ${overview.overdueCount} 条已过期` : '没有过期的' }}
          </span>
        </div>
        <div class="rp-card">
          <span class="rp-num">
            {{ overview.avgStayDays === null ? '—' : overview.avgStayDays }}
          </span>
          <span class="rp-label">平均停留</span>
          <span class="rp-sub">
            {{ overview.avgStayDays === null ? '这段时间没完成过' : '天（从记下到干完）' }}
          </span>
        </div>
      </div>

      <!-- 专注：番茄钟那边记下来的，进不了待办文件（那是三方共用的），所以单独一卡 -->
      <div class="rp-card rp-card-wide">
        <div class="rp-focus">
          <Timer :size="15" :stroke-width="2" class="rp-focus-icon" />
          <span class="rp-num sm">{{ focusCount }}</span>
          <span class="rp-label">个番茄 · {{ fmtMinutes(focusMinutes) }}</span>
        </div>
        <span class="rp-sub">
          {{ focusCount === 0 ? '这段时间还没专注过（去番茄闹钟开一轮）' : `${range.label}内的专注时长` }}
        </span>
      </div>

      <!-- 按领域：完成 / 总数 + 完成率 + 占比条 -->
      <section class="rp-sec">
        <h3 class="rp-h3">按领域</h3>
        <p v-if="domains.length === 0" class="rp-none">这段时间没有任何动静</p>
        <div v-for="d in domains" :key="d.domain" class="rp-row">
          <span class="rp-row-name" :title="domainName(d.domain)">{{ domainName(d.domain) }}</span>
          <span class="rp-row-num">{{ d.done }}<i>/{{ d.total }}</i></span>
          <span class="rp-row-rate">{{ d.rate }}%</span>
          <span class="rp-bar"><i :style="{ width: `${barWidth(d.done)}%` }" /></span>
        </div>
      </section>

      <!-- 还欠着的：过期条数标红，那是最该先动的一批 -->
      <section class="rp-sec">
        <h3 class="rp-h3">还在欠着</h3>
        <p v-if="pending.length === 0" class="rp-none">清空了，一条不欠</p>
        <div v-for="p in pending" :key="p.domain" class="rp-row">
          <span class="rp-row-name" :title="domainName(p.domain)">{{ domainName(p.domain) }}</span>
          <span class="rp-row-num">{{ p.count }}</span>
          <span class="rp-row-rate">
            <em v-if="p.overdue > 0" class="rp-overdue">{{ p.overdue }} 条过期</em>
          </span>
          <span class="rp-bar muted"><i :style="{ width: `${pendingWidth(p.count)}%` }" /></span>
        </div>
      </section>

      <!-- 小刀那摊：按状态分布，排序跟小刀视图一致 -->
      <section class="rp-sec">
        <h3 class="rp-h3">小刀那摊</h3>
        <p v-if="agent.length === 0" class="rp-none">小刀手上没有活</p>
        <div v-else class="rp-tags">
          <span v-for="a in agent" :key="a.status" class="rp-tag" :class="`st-${a.status}`">
            {{ STATUS_LABELS[a.status] ?? a.status }}
            <b>{{ a.count }}</b>
          </span>
        </div>
      </section>

      <p class="rp-note">按「{{ today }}」算 · 已移出列表的不计入</p>
    </template>
  </div>
</template>

<style scoped>
.rp {
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
}

.rp-head {
  flex: none;
  display: flex;
  align-items: center;
  gap: 9px;
  flex-wrap: wrap;
}

/* 分段控件：跟设置页那套同一个语言（.sv-btn / .on） */
.rp-seg {
  flex: none;
  display: inline-flex;
  padding: 2px;
  border-radius: 999px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
}

.rp-seg-btn {
  padding: 3px 13px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.rp-seg-btn:hover {
  color: var(--xd-text);
}

.rp-seg-btn.on {
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}

.rp-span {
  margin: 0;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 数字卡：窄窗口下自动折列（360px 时是一列两列交替，不至于挤成一条） */
.rp-cards {
  flex: none;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
  gap: 8px;
}

.rp-card {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 9px 11px 8px;
  border-radius: var(--xd-radius);
  background: var(--xd-card);
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-card);
}

.rp-card-wide {
  flex: none;
}

.rp-num {
  /* 等宽数字：几行数字上下要对得齐，比例字体会让"1"比"8"窄一截 */
  font-variant-numeric: tabular-nums;
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(24px * var(--xd-font-scale));
  font-weight: 600;
  line-height: 1.15;
  color: var(--xd-text);
}

.rp-num.sm {
  font-size: calc(20px * var(--xd-font-scale));
}

.rp-num.warn {
  color: var(--xd-red);
}

.rp-label {
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text-sub);
}

.rp-sub {
  font-size: calc(12px * var(--xd-font-scale));
  line-height: 1.45;
  color: var(--xd-text-dim);
}

.rp-focus {
  display: flex;
  align-items: baseline;
  gap: 6px;
  flex-wrap: wrap;
}

.rp-focus-icon {
  align-self: center;
  color: var(--xd-accent);
}

.rp-sec {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.rp-h3 {
  margin: 2px 0 0;
  font-size: calc(13.2px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text-sub);
}

.rp-none {
  margin: 0;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 一行三段：名字（可截断）+ 数字（不换行）+ 条 */
.rp-row {
  display: flex;
  align-items: center;
  gap: 7px;
}

.rp-row-name {
  flex: none;
  width: calc(64px * var(--xd-font-scale));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-sub);
}

.rp-row-num {
  flex: none;
  min-width: calc(34px * var(--xd-font-scale));
  font-variant-numeric: tabular-nums;
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text);
}

.rp-row-num i {
  font-style: normal;
  color: var(--xd-text-dim);
}

.rp-row-rate {
  flex: none;
  min-width: calc(46px * var(--xd-font-scale));
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.rp-overdue {
  font-style: normal;
  color: var(--xd-red);
}

.rp-bar {
  flex: 1;
  min-width: 0;
  height: calc(6px * var(--xd-font-scale));
  border-radius: 999px;
  background: var(--xd-accent-soft);
  overflow: hidden;
}

.rp-bar i {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--xd-accent);
  transition: width 0.25s var(--xd-ease);
}

/* 欠着的那组用更弱的填充：它是"待办"不是"成绩"，别跟完成率抢眼 */
.rp-bar.muted {
  background: var(--xd-border-soft);
}

.rp-bar.muted i {
  background: var(--xd-text-dim);
}

.rp-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.rp-tag {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: var(--xd-card);
  color: var(--xd-text-sub);
  font-size: calc(12.6px * var(--xd-font-scale));
}

.rp-tag b {
  font-variant-numeric: tabular-nums;
  color: var(--xd-text);
}

/* 状态色跟待办面板上那排 chip 同一套取值，两处认起来是同一个东西 */
.rp-tag.st-progress {
  border-color: color-mix(in srgb, var(--xd-accent) 45%, transparent);
  color: var(--xd-accent);
}

.rp-tag.st-waiting {
  border-color: color-mix(in srgb, var(--xd-w-high) 45%, transparent);
  color: var(--xd-w-high);
}

.rp-note {
  flex: none;
  margin: 2px 0 0;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.rp-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 34px 16px;
  text-align: center;
}

.rp-empty-mark {
  margin: 0;
  font-size: calc(26.4px * var(--xd-font-scale));
  opacity: 0.7;
}

.rp-empty-title {
  margin: 0;
  font-size: calc(15px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text-sub);
}

.rp-empty-desc {
  margin: 0;
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}
</style>
