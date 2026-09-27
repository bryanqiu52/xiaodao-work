<script setup lang="ts">
// 番茄闹钟主界面 —— **只负责渲染**。
//
// 状态与计时在 `composables/useFocus.ts`（模块级单例），由 App 启动，
// 所以切到别的工具、甚至窗口藏进托盘，计时照样跑、到点照样通知。
// 这里拿到的 `focusNow` 是那个节拍器推进的时间戳：回到这一页时
// 剩余时间按 `deadline - now` 一步算准，不需要"补算"离开期间。
import { computed, ref } from 'vue'
import { Link2, Pause, Play, RotateCcw, SkipForward, X } from 'lucide-vue-next'
import {
  focusPrimary,
  focusProgress,
  focusRemaining,
  focusReset,
  focusSkip,
  focusState,
  focusTaskId,
  focusTaskTitle,
  focusTodayCount,
  setFocusTask,
} from '../composables/useFocus'
import { fmtMinutes, focusStatsOfTodo } from '../composables/useFocusLog'
import { configStore, saveConfig } from '../stores/config'
import { todoStore } from '../stores/todo'
import TaskPicker from './TaskPicker.vue'

const PHASE_LABEL = { work: '专注', short: '短休息', long: '长休息' } as const

const state = focusState
const remain = focusRemaining

const timeLabel = computed(() => {
  // ceil：剩 0.2 秒也显示 0:01，别在最后半秒显示 0:00 却还没到点
  const totalSec = Math.ceil(remain.value / 1000)
  const m = Math.floor(totalSec / 60)
  const sec = totalSec % 60
  return `${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
})

const isWork = computed(() => state.value.phase === 'work')
const runLabel = computed(() => {
  if (state.value.run === 'running') return '进行中'
  if (state.value.run === 'paused') return '已暂停'
  return '待开始'
})
const primaryLabel = computed(() =>
  state.value.run === 'running' ? '暂停' : state.value.run === 'paused' ? '继续' : '开始',
)

// 进度环
const R = 88
const CIRC = 2 * Math.PI * R
const dashOffset = computed(() => CIRC * (1 - focusProgress.value))

// ── 页内快捷设置 ──────────────────────────────────────────────────────────
// 最常用的两项就摆在钟下面，不用进设置页翻（完整的六项在「设置 → 工具 → 番茄闹钟」）。
const WORK_STEPS = [15, 20, 25, 30, 45, 60]
const ROUND_STEPS = [2, 3, 4, 5, 6]

async function setWork(v: number): Promise<void> {
  await saveConfig({ focus_work_min: v })
}

async function setRounds(v: number): Promise<void> {
  await saveConfig({ focus_rounds: v })
}

// ── 挂在哪条待办上 ────────────────────────────────────────────────────────

const pickerOpen = ref(false)
const bound = computed(() => focusTaskId.value !== '')
/** 这条待办累计的番茄数与分钟数（来自专注记录，不是本次会话） */
const taskStats = computed(() => focusStatsOfTodo(focusTaskId.value))

/** 单选浮层只回一个 id；拿标题去清单里查（绑的时候顺手存下标题，见 useFocus） */
function onPickTask(ids: string[]): void {
  pickerOpen.value = false
  const id = ids[0]
  if (id === undefined) return
  const item = todoStore.items.find((i) => i.id === id)
  if (item) setFocusTask(item)
}
</script>

<template>
  <div class="focus">
    <div class="focus-box" :class="{ rest: !isWork }">
      <div class="focus-phase">
        <span class="focus-phase-name">{{ PHASE_LABEL[state.phase] }}</span>
        <span class="focus-round">
          {{
            isWork
              ? `第 ${state.round + 1} 轮 · 每 ${configStore.cfg.focus_rounds} 轮长休`
              : '养养眼睛，动一动'
          }}
        </span>
      </div>

      <!-- 挂在哪条待办上。**休息阶段也留着** —— 下一轮专注多半还是同一条，
           休息时被清掉的话每轮都要重选一次。
           绑定只是运行态（不落盘），重启回到"未关联"，跟番茄钟本身一个口径。 -->
      <div class="focus-task">
        <template v-if="bound">
          <span class="ft-label">正在专注</span>
          <button
            type="button"
            class="ft-name"
            :title="`${focusTaskTitle}（点一下换一条）`"
            @click="pickerOpen = true"
          >
            {{ focusTaskTitle }}
          </button>
          <span v-if="taskStats.count > 0" class="ft-stats">
            累计 {{ taskStats.count }} 个 · {{ fmtMinutes(taskStats.minutes) }}
          </span>
          <button
            type="button"
            class="ft-x"
            aria-label="取消关联"
            title="取消关联"
            @click="setFocusTask(null)"
          >
            <X :size="12" :stroke-width="2.4" />
          </button>
        </template>
        <button v-else type="button" class="ft-add" @click="pickerOpen = true">
          <Link2 :size="12" :stroke-width="2" />
          未关联待办 · 选一条
        </button>
      </div>

      <div
        class="focus-dial"
        role="timer"
        :aria-label="`${PHASE_LABEL[state.phase]}倒计时 ${timeLabel}`"
      >
        <svg viewBox="0 0 200 200" aria-hidden="true">
          <circle class="dial-track" cx="100" cy="100" :r="R" />
          <circle
            class="dial-fg"
            cx="100"
            cy="100"
            :r="R"
            :stroke-dasharray="CIRC"
            :stroke-dashoffset="dashOffset"
          />
        </svg>
        <div class="dial-center">
          <span class="dial-time">{{ timeLabel }}</span>
          <span class="dial-sub">{{ runLabel }}</span>
        </div>
      </div>

      <div class="focus-actions">
        <button type="button" class="focus-primary" @click="focusPrimary">
          <Pause v-if="state.run === 'running'" :size="15" :stroke-width="2.2" />
          <Play v-else :size="15" :stroke-width="2.2" />
          {{ primaryLabel }}
        </button>
        <button type="button" class="sv-btn" :disabled="state.run === 'idle'" @click="focusSkip">
          <SkipForward :size="13" :stroke-width="2" />
          跳过
        </button>
        <button type="button" class="sv-btn" @click="focusReset">
          <RotateCcw :size="13" :stroke-width="2" />
          重置
        </button>
      </div>

      <p class="focus-today">
        今天完成 <b>{{ focusTodayCount }}</b> 个番茄
      </p>
    </div>

    <!-- 页内快捷设置：切到别的页面计时照样跑，这两项改完下一阶段生效 -->
    <div class="focus-quick">
      <div class="fq-row">
        <span class="fq-label">专注时长</span>
        <div class="fq-chips" role="group" aria-label="专注时长">
          <button
            v-for="v in WORK_STEPS"
            :key="v"
            type="button"
            class="fq-chip"
            :class="{ on: configStore.cfg.focus_work_min === v }"
            @click="setWork(v)"
          >
            {{ v }} 分
          </button>
        </div>
      </div>
      <div class="fq-row">
        <span class="fq-label">长休轮数</span>
        <div class="fq-chips" role="group" aria-label="长休前的轮数">
          <button
            v-for="v in ROUND_STEPS"
            :key="v"
            type="button"
            class="fq-chip"
            :class="{ on: configStore.cfg.focus_rounds === v }"
            @click="setRounds(v)"
          >
            {{ v }} 轮
          </button>
        </div>
      </div>
      <p class="fq-note">改完下一阶段生效，正在跑的这一轮不受影响</p>
    </div>

    <!-- 浮层自己 Teleport 到 body，挂在哪儿都行 -->
    <TaskPicker
      :open="pickerOpen"
      title="专注挂到哪条待办上"
      @pick="onPickTask"
      @close="pickerOpen = false"
    />
  </div>
</template>

<style scoped>
.focus {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 14px 12px;
  overflow-y: auto;
}

.focus-box {
  --dial-color: var(--xd-accent);
  flex: none;
  /* 垂直居中靠"首尾自动外边距"，不用 justify-content: center ——
     后者在内容比容器高时会把顶部裁掉（flexbox 的经典坑），自动外边距不会 */
  margin-top: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  width: min(100%, 320px);
  padding: 24px 26px 18px;
  border-radius: var(--xd-radius-lg);
  background: var(--xd-card);
  border: 1px solid var(--xd-border);
  /* 卡面底色走 --xd-card：卡片不透明度、沉浸模式的毛玻璃对它照常生效 ——
     番茄钟不该是"不跟着主题走"的孤岛 */
}

/* 专注 = 强调色，休息 = 绿。切换阶段时颜色一起过渡，不用读字也看得出状态 */
.focus-box.rest {
  --dial-color: var(--xd-green);
}

.focus-phase {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
}

.focus-phase-name {
  font-size: calc(17px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--dial-color);
}

.focus-round {
  font-size: calc(12.4px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 挂在哪条待办上。**克制**：一行小字 + 两个小按钮，
   它是给钟做注脚的，不该抢进度环的注意力 */
.focus-task {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 5px;
  max-width: 100%;
  font-size: calc(12.4px * var(--xd-font-scale));
}

.ft-label {
  flex: none;
  color: var(--xd-text-dim);
}

/* 标题可能很长：压缩省略，完整的那份在 title 里 */
.ft-name {
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 1px 7px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-size: calc(12.4px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.ft-name:hover {
  border-color: var(--xd-accent);
}

.ft-stats {
  flex: none;
  color: var(--xd-text-dim);
}

.ft-x {
  flex: none;
  display: grid;
  place-items: center;
  width: calc(19px * var(--xd-font-scale));
  height: calc(19px * var(--xd-font-scale));
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--xd-text-dim);
  transition: all 0.14s var(--xd-ease);
}

.ft-x:hover {
  background: color-mix(in srgb, var(--xd-red) 15%, transparent);
  color: var(--xd-red);
}

/* 没绑的时候是个虚线"空位"：一眼看出这里还能填东西 */
.ft-add {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 10px;
  border: 1px dashed var(--xd-border);
  border-radius: 999px;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(12.4px * var(--xd-font-scale));
  line-height: calc(18.6px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.ft-add:hover {
  border-style: solid;
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}

.focus-dial {
  position: relative;
  width: 208px;
  height: 208px;
}

.focus-dial svg {
  width: 100%;
  height: 100%;
}

.dial-track {
  fill: none;
  stroke: var(--xd-border);
  stroke-width: 7;
}

.dial-fg {
  fill: none;
  stroke: var(--dial-color);
  stroke-width: 7;
  stroke-linecap: round;
  /* 从 12 点方向起画 */
  transform: rotate(-90deg);
  transform-origin: center;
  transition: stroke-dashoffset 0.25s linear;
}

.dial-center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
}

.dial-time {
  font-family: 'Cascadia Code', Consolas, monospace;
  font-variant-numeric: tabular-nums;
  font-size: calc(40px * var(--xd-font-scale));
  font-weight: 600;
  line-height: 1.1;
  color: var(--xd-text);
}

.dial-sub {
  font-size: calc(12.4px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.focus-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.focus-primary {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 8px 22px;
  border-radius: 999px;
  border: none;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-size: calc(14.2px * var(--xd-font-scale));
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease-out;
}

.focus-primary:hover {
  background: var(--xd-accent-hover);
}

.focus-today {
  margin: 0;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.focus-today b {
  color: var(--xd-text);
  font-variant-numeric: tabular-nums;
}

/* ── 页内快捷设置 ─────────────────────────────────────────────────────── */

.focus-quick {
  flex: none;
  margin-bottom: auto; /* 与上面卡片的 margin-top: auto 配合，把整组推到垂直中间 */
  width: min(100%, 320px);
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding: 12px 14px;
  border-radius: var(--xd-radius-lg);
  background: var(--xd-card);
  border: 1px solid var(--xd-border);
}

.fq-row {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.fq-label {
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.fq-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.fq-chip {
  flex: none;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid var(--xd-border);
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(12.2px * var(--xd-font-scale));
  cursor: pointer;
  transition:
    border-color 0.15s,
    color 0.15s,
    background 0.15s;
}

.fq-chip:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.fq-chip.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}

.fq-note {
  margin: 0;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  line-height: 1.5;
}
</style>
