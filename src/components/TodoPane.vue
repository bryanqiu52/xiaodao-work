<script setup lang="ts">
// 待办面板主视图：外卡三段竖排 —— 顶栏 / 自滚列表 / 底部常驻快记，**只有列表自滚**。
//
// 计数口径（照搬插件，改之前先看 view.ts 里的说明）：
//   - 顶部 tab 的数字 = 该归属未完成数，不受任何筛选影响；
//   - 权重 chip 的数字 = 归属 + 领域（不含权重自身）；
//   - 状态 chip 的数字 = 归属 + 领域 + 权重（不含状态自身）。
// 所以选中一档之后，别的选项数字不会缩水，方便直接改选。
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { CalendarClock, Search, SlidersHorizontal, Undo2, X } from 'lucide-vue-next'
import OwnerTabs from './OwnerTabs.vue'
import TodoRow from './TodoRow.vue'
import ReviewRow from './ReviewRow.vue'
import CompletedRow from './CompletedRow.vue'
import EditRow from './EditRow.vue'
import Composer from './Composer.vue'
import TransferForm from './TransferForm.vue'
import FilterSheet from './FilterSheet.vue'
import { logToBackend, onTodoFileChanged } from '../api/tauri'
import { snoozeItem, SNOOZE_HOURS } from '../composables/useReminder'
import { applyPatch } from '../core/patch'
import { createItem, markDeleted, markDone, recordReview, recordTransfer, restoreItem } from '../core/rules'
import { draftToInput, draftToPatch, emptyDraft, type DraftFields } from '../core/draft'
import { FLASH_MS, OWNER_AGENT, PRIORITY_LABELS, STATUS_FILTER_OPTIONS, STATUS_LABELS } from '../core/constants'
import { useDomains } from '../composables/useDomains'

const { label: domainName } = useDomains()
import {
  completedItems,
  countBy,
  countPendingOwned,
  groupByDueBuckets,
  matchesDomain,
  matchesKeyword,
  matchesPriority,
  matchesStatus,
  visibleItems,
} from '../core/view'
import type { ReviewAction, Status, TodoItem } from '../core/types'
import { commit, loadTodos, todoStore, undo, undoState } from '../stores/todo'
import { configStore } from '../stores/config'

// ── 视图状态 ──────────────────────────────────────────────────────────────

const owner = ref(configStore.cfg.default_view === OWNER_AGENT ? OWNER_AGENT : 'user')
const keyword = ref('')
const domain = ref<string | null>(null)
const priority = ref<string | null>(null)
const status = ref<string | null>(null)
/**
 * 「已完成」勾选，**默认不勾**。
 *
 * 勾着 = **只看已完成**（切过去就是为了恢复或删除，所以要一份干净的清单）；
 * 取消 = 回到未完成。放在筛选按钮右边，一眼能找到，不用点开抽屉。
 * 默认关着 —— 待办列表的主角是"还没做完的事"，已完成想看再勾。
 * 注意它**不吃**搜索 / 领域 / 权重之外的东西：这几个条件照样生效（见 completed）。
 */
const showDone = ref(false)
/**
 * 「按期限看」开关。**默认关着** —— 现有那套（权重优先）是跑熟的手感，
 * 分组只该是"想按期限收拾一遍时"临时拧过去的，不该一上来就改变所有人的默认视图。
 * 纯视图状态，不落盘。
 */
const dueMode = ref(false)
const sheetOpen = ref(false)
const editingId = ref<string | null>(null)
const transferringId = ref<string | null>(null)

const flash = ref('')
let flashTimer: number | undefined
function showFlash(text: string): void {
  flash.value = text
  if (flashTimer !== undefined) window.clearTimeout(flashTimer)
  flashTimer = window.setTimeout(() => {
    flash.value = ''
  }, FLASH_MS)
}

const libRoot = computed(() => configStore.cfg.library_root)
const agentView = computed(() => owner.value === OWNER_AGENT)
const hint = computed(() =>
  agentView.value
    ? '点状态徽章改状态（暂停 / 排队最常用）· 点 ✎ 修改 · 点 ✓ 完成'
    : '在下面写一句回车就记下 · 点 ✎ 修改 · 点 ✓ 完成',
)

// ── 数据切片 ──────────────────────────────────────────────────────────────

/** 底数：默认排除已删除（设置里可打开） */
const base = computed<TodoItem[]>(() =>
  configStore.cfg.show_deleted
    ? todoStore.items
    : todoStore.items.filter((i) => i.deletedAt === null),
)

/** 该归属下未完成、已排好序（还没套搜索 / 领域 / 权重 / 状态） */
const pool = computed(() => visibleItems(base.value, owner.value))

const weightPool = computed(() =>
  pool.value.filter((i) => matchesKeyword(i, keyword.value.trim()) && matchesDomain(i, domain.value)),
)
const statusPool = computed(() => weightPool.value.filter((i) => matchesPriority(i, priority.value)))

const visible = computed(() => statusPool.value.filter((i) => matchesStatus(i, status.value)))

// ── 期限分组 ──────────────────────────────────────────────────────────────

/** 按期限切成「已过期 / 今天 / 本周 / 以后 / 没期限」。组内顺序仍是原来那套 */
const dueGroups = computed(() => groupByDueBuckets(visible.value))

/**
 * 渲染用的扁平行列表。
 *
 * **为什么把分组标题挂在"该组第一行"上，而不是分两层循环**：分两层就得把
 * 那一大段行内模板（TodoRow / ReviewRow / TransferForm 三选一 + 转移表单）
 * 抄第二遍，两处迟早对不上。扁平化之后模板只写一遍，标题跟着它的第一行走。
 */
interface RenderRow {
  item: TodoItem
  /** 分组标题，只挂每组第一行；不分组时是 null */
  header: string | null
  /** 分组的用途色标记（见样式里的 .due-*） */
  headerTone: string
}

const renderRows = computed<RenderRow[]>(() => {
  const out: RenderRow[] = []
  if (!dueMode.value) {
    for (const it of visible.value) out.push({ item: it, header: null, headerTone: '' })
    return out
  }
  for (const g of dueGroups.value) {
    g.items.forEach((it, idx) => {
      out.push({
        item: it,
        header: idx === 0 ? `${g.label} · ${g.items.length} 条` : null,
        headerTone: g.bucket,
      })
    })
  }
  return out
})

// ── 撤销 ──────────────────────────────────────────────────────────────────

/**
 * 撤销上一步。走 store 的 `undo()`，它按当前指纹写回上一版 ——
 * 盘上被外部改过就会被挡下（那时栈也清空了，不会去覆盖 AI 的改动）。
 */
async function doUndo(): Promise<void> {
  const err = await undo()
  showFlash(err ?? `已撤销：${undoState.label}`)
}

/**
 * 「已完成」清单：**和未完成那份共用一套筛选**（搜索 / 领域 / 权重）。
 * 勾上「已完成」再搜一个词，要的是"已完成里含这个词的那几条"，
 * 不是"又把全部已完成倒出来" —— 否则搜了半天等于没搜。
 *
 * 状态筛选不参与：那排 chip 在已完成视图里本来就不显示，且切换时会被清掉。
 */
const completed = computed(() =>
  completedItems(base.value, owner.value).filter(
    (i) =>
      matchesKeyword(i, keyword.value.trim()) &&
      matchesDomain(i, domain.value) &&
      matchesPriority(i, priority.value),
  ),
)

/** 该归属下已完成的**总数**（不受筛选影响，用于在筛选时说清"筛出多少 / 共多少"） */
const completedTotal = computed(() => completedItems(base.value, owner.value).length)

/**
 * 搜索框里有没有词 —— **有词就进入全局搜索模式**。
 *
 * 全局搜索**只认关键词这一个条件**：不套归属、不套「已完成」，也不套领域 / 权重 / 状态。
 * 为什么：这几层筛子叠起来就会出现"明明有这条却搜不到" —— 它被归在另一个归属里，
 * 或者已经完成了。用户搜不到只会以为数据丢了（这次就是这么发生的）。
 * 搜索的目的是**把东西找出来**，不是"在当前这批里再筛一遍"。
 *
 * 已删除的仍然排除 —— 那是用户主动移出的，不该混进结果里（想找它有「显示已删除」）。
 */
const searching = computed(() => keyword.value.trim() !== '')

const hits = computed(() => {
  const kw = keyword.value.trim()
  if (!kw) return { pending: [] as TodoItem[], done: [] as TodoItem[] }
  const found = base.value.filter((i) => matchesKeyword(i, kw))
  return {
    pending: found.filter((i) => i.status !== 'done'),
    done: found.filter((i) => i.status === 'done'),
  }
})

/**
 * 中文输入法：**拼字期间不要拿半成品去搜**。
 * 输入过程中 `input` 事件会把拼音字母（"zhang"）也喂进来，搜出来的全是莫名其妙的东西。
 * 等 `compositionend`（上屏）再更新关键词 —— 参考实现里踩过同一个坑。
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

const counts = computed(() => ({
  user: countPendingOwned(base.value, 'user'),
  agent: countPendingOwned(base.value, OWNER_AGENT),
}))

const statusCounts = computed(() => countBy(statusPool.value, 'status'))
const filterCount = computed(
  () =>
    (domain.value ? 1 : 0) +
    (priority.value ? 1 : 0) +
    (status.value ? 1 : 0) +
    (keyword.value.trim() ? 1 : 0),
)

const activeChips = computed(() => {
  const out: { key: string; label: string; clear: () => void }[] = []
  const kw = keyword.value.trim()
  if (kw) out.push({ key: 'kw', label: `搜「${kw}」`, clear: () => (keyword.value = '') })
  if (domain.value)
    out.push({
      key: 'domain',
      label: domainName(domain.value),
      clear: () => (domain.value = null),
    })
  if (priority.value)
    out.push({
      key: 'priority',
      label: `权重 ${PRIORITY_LABELS[priority.value] ?? priority.value}`,
      clear: () => (priority.value = null),
    })
  if (status.value)
    out.push({
      key: 'status',
      label: STATUS_LABELS[status.value] ?? status.value,
      clear: () => (status.value = null),
    })
  return out
})

function clearAllFilters(): void {
  keyword.value = ''
  domain.value = null
  priority.value = null
  status.value = null
}

// 切到我的视图 / 切到已完成时清掉状态筛选（状态筛选只在小刀视图有意义）
watch([owner, showDone], () => {
  status.value = null
})
// 编辑中的条目被筛掉时自动收起表单，避免"改完看不到自己改的那条"
watch(visible, (list) => {
  if (editingId.value && !list.some((i) => i.id === editingId.value)) editingId.value = null
  if (transferringId.value && !list.some((i) => i.id === transferringId.value)) transferringId.value = null
})

// ── 写操作：一律走 commit，失败（冲突）时 UI 纹丝不动 ──────────────────────

async function run(mutate: (draft: TodoItem[]) => void, okText: string): Promise<void> {
  // okText 同时也是撤销按钮上那句人话（「移出列表」「状态改为「进行中」」）——
  // 两处要的是同一件事："这一步干了什么"，没必要各写一份
  const ok = await commit(mutate, okText)
  showFlash(ok ? okText : '外部有改动，已按盘上最新的刷新')
}

async function addDraft(d: DraftFields): Promise<void> {
  const input = draftToInput(d)
  await run((draft) => draft.push(createItem(input)), '记下了')
}

/**
 * 「稍后再说」：把这条推到 4 小时后再提醒。
 * 注意它**不改待办本身**（不动 dueAt、不动状态）—— 推迟的只是"什么时候再提醒你"，
 * 是个提醒层面的概念，不该污染待办数据（那份是三方共用的）。
 */
async function snooze(item: TodoItem): Promise<void> {
  await snoozeItem(item.id)
  showFlash(`${item.title} 已推后 ${SNOOZE_HOURS} 小时再提醒`)
}

async function toggleDone(item: TodoItem): Promise<void> {
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) markDone(it)
  }, '已完成')
}

async function softRemove(item: TodoItem): Promise<void> {
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) markDeleted(it)
  }, '已移出列表')
}

async function restore(item: TodoItem): Promise<void> {
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) restoreItem(it)
  }, '已恢复成未完成')
}

async function changeStatus(item: TodoItem, next: Status): Promise<void> {
  if (next === item.status) return
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) applyPatch(it, { status: next })
  }, `状态改为「${STATUS_LABELS[next] ?? next}」`)
}

async function saveEdit(item: TodoItem, d: DraftFields): Promise<void> {
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) applyPatch(it, draftToPatch(d))
  }, '已保存')
  editingId.value = null
}

async function transfer(item: TodoItem, to: 'user' | 'agent', note: string): Promise<void> {
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) recordTransfer(it, to, note)
  }, to === 'agent' ? '已交给小刀' : '已转给你自己')
  transferringId.value = null
}

async function review(item: TodoItem, action: ReviewAction, comment: string): Promise<void> {
  await run((draft) => {
    const it = draft.find((i) => i.id === item.id)
    if (it) recordReview(it, action, comment)
  }, action === 'approved' ? '已通过' : action === 'rejected' ? '已退回给小刀' : '已挂起')
}

// ── 外部改动 ──────────────────────────────────────────────────────────────

/**
 * `Ctrl+Z` 撤销。
 *
 * **必须跳过正在输入的地方**：在标题框里按 Ctrl+Z，用户要的是"撤销这行字"，
 * 不是"撤销上一条待办" —— 抢过来会把正在编辑的内容连人一起搞乱。
 */
function onKeydown(e: KeyboardEvent): void {
  if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return
  if (e.key !== 'z' && e.key !== 'Z') return
  const el = e.target as HTMLElement | null
  if (el !== null && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)) return
  if (undoState.size === 0) return
  e.preventDefault()
  void doUndo()
}

let unlisten: (() => void) | undefined
onMounted(async () => {
  unlisten = await onTodoFileChanged(() => {
    logToBackend('info', '收到 todo-file-changed，重载清单')
    void loadTodos()
  })
  if (!todoStore.fingerprint) await loadTodos()
  window.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => {
  unlisten?.()
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="pane">
    <div class="pane-head">
      <OwnerTabs
        :owner="owner"
        :counts="counts"
        :loaded="!todoStore.loading"
        @change="owner = $event"
      />
      <!-- 提示与撤销同一行：撤销是"刚做完那一下的后悔药"，
           摆在提示旁边最顺手，也不占筛选那一行的位置 -->
      <div class="pane-toolbar">
        <p class="pane-hint">{{ hint }}</p>
        <button
          v-if="undoState.size > 0"
          type="button"
          class="pane-undo"
          :title="`撤销上一步：${undoState.label}（快捷键 Ctrl+Z）`"
          @click="doUndo"
        >
          <Undo2 :size="12" :stroke-width="2.2" />
          撤销
        </button>
      </div>

      <p v-if="flash" class="pane-flash">{{ flash }}</p>
      <p v-else-if="todoStore.error" class="pane-error">{{ todoStore.error }}</p>

      <div class="pane-filters">
        <div class="pane-search">
          <Search :size="13" :stroke-width="2" class="pane-search-icon" />
          <!-- 不用 v-model：中文输入法拼字期间会把拼音字母喂进来（见 onSearchInput 的说明）。
               Esc 仍然直接清空 —— 那时候没有未上屏的候选词，不用走 composing 那套 -->
          <input
            :value="keyword"
            class="pane-search-input"
            type="text"
            placeholder="全局搜：标题 / 编号 / 正文"
            spellcheck="false"
            @input="onSearchInput"
            @compositionstart="composing = true"
            @compositionend="onCompositionEnd"
            @keydown.esc="keyword = ''"
          />
          <button
            v-if="keyword"
            class="pane-search-clear"
            type="button"
            aria-label="清空搜索"
            @click="keyword = ''"
          >
            <X :size="12" :stroke-width="2" />
          </button>
        </div>
        <button type="button" class="pane-filter-btn" @click="sheetOpen = true">
          <SlidersHorizontal :size="13" :stroke-width="2" />
          筛选
          <!-- 搜索时那几个条件不生效，数字就没意义了 —— 别挂着一个会误导人的角标 -->
          <span v-if="filterCount && !searching" class="pane-filter-n">{{ filterCount }}</span>
        </button>
        <!-- 按期限分组：图标按钮，窄窗口下不跟文字抢地方。
             搜索态与已完成态不分组（那两份清单的语义不同，硬分组只会更乱） -->
        <button
          type="button"
          class="pane-filter-btn pane-icon-btn"
          :class="{ on: dueMode }"
          :aria-pressed="dueMode"
          :disabled="searching || showDone"
          :title="
            searching || showDone
              ? '搜索 / 已完成清单不分组'
              : dueMode
                ? '按期限分组中（点一下回到权重优先）'
                : '按期限分组：已过期 / 今天 / 本周 / 以后 / 没期限'
          "
          @click="dueMode = !dueMode"
        >
          <CalendarClock :size="13" :stroke-width="2" />
        </button>
        <!-- 「已完成」勾选：原插件就摆在筛选右边。勾上 = **只看已完成**，
             取消 = 回到未完成清单（两个是互斥的两份清单，不是叠加）。 -->
        <button
          type="button"
          class="pane-chip pane-chip-done"
          :class="{ on: showDone }"
          :aria-pressed="showDone"
          :disabled="searching"
          :title="
            searching
              ? '搜索时不分未完成 / 已完成（都已包含），清空搜索后恢复'
              : showDone
                ? '回到未完成清单'
                : '只看已完成的'
          "
          @click="showDone = !showDone"
        >
          已完成
          <span class="pane-chip-n">{{ completed.length }}</span>
        </button>
      </div>

      <!-- 状态筛选只在小刀视图出现 -->
      <div v-if="agentView && !showDone" class="pane-chips">
        <button
          type="button"
          class="pane-chip"
          :class="{ on: status === null }"
          @click="status = null"
        >
          全部
        </button>
        <button
          v-for="s in STATUS_FILTER_OPTIONS"
          :key="s"
          type="button"
          class="pane-chip"
          :class="[`st-${s}`, { on: status === s }]"
          @click="status = s"
        >
          {{ STATUS_LABELS[s] }}
          <span class="pane-chip-n">{{ statusCounts[s] ?? 0 }}</span>
        </button>
      </div>

      <div v-if="activeChips.length" class="pane-active">
        <button
          v-for="c in activeChips"
          :key="c.key"
          type="button"
          class="pane-active-chip"
          @click="c.clear()"
        >
          {{ c.label }}
          <X :size="10" :stroke-width="2.5" />
        </button>
        <span class="pane-active-n">{{ visible.length }} 条</span>
        <button type="button" class="pane-active-clear" @click="clearAllFilters">清除</button>
      </div>
    </div>

    <div class="pane-list">
      <div v-if="todoStore.loading" class="pane-empty">
        <p class="pane-empty-mark">⏳</p>
        <p class="pane-empty-title">正在同步…</p>
      </div>

      <!-- 全局搜索：**只认关键词**，跨归属、含已完成。
           分组 + 归属标签是必须的 —— 结果里混着"我的 / 小刀的"和"已完成"，
           不标清楚，用户会以为搜出来的都是自己未做的事 -->
      <template v-else-if="searching">
        <p class="pane-done-tip">
          全局搜「{{ keyword.trim() }}」· 跨归属、含已完成 · 其它筛选条件暂不生效
        </p>

        <template v-if="hits.pending.length > 0">
          <p class="pane-group-tip">未完成 {{ hits.pending.length }} 条</p>
          <div v-for="it in hits.pending" :key="it.id" class="pane-item">
            <TodoRow
              :item="it"
              :agent-view="it.owner === OWNER_AGENT"
              :lib-root="libRoot"
              show-owner
              @done="toggleDone(it)"
              @snooze="snooze(it)"
              @remove="softRemove(it)"
              @edit="editingId = it.id"
              @status="changeStatus(it, $event)"
              @transfer="transferringId = transferringId === it.id ? null : it.id"
              @result="showFlash($event)"
            />
          </div>
        </template>

        <template v-if="hits.done.length > 0">
          <p class="pane-group-tip">已完成 {{ hits.done.length }} 条</p>
          <CompletedRow
            v-for="it in hits.done"
            :key="it.id"
            :item="it"
            show-owner
            @restore="restore(it)"
            @remove="softRemove(it)"
          />
        </template>

        <div v-if="hits.pending.length === 0 && hits.done.length === 0" class="pane-empty">
          <p class="pane-empty-mark">🔍</p>
          <p class="pane-empty-title">全部清单里都没有匹配的</p>
          <p class="pane-empty-desc">「我的」和「小刀的」、含已完成都搜过了</p>
          <button type="button" class="pane-empty-btn" @click="keyword = ''">清空搜索</button>
        </div>
      </template>

      <!-- 勾上「已完成」= **只看已完成**（不是"混在列表后面接着显示"）：
           切过去就是为了恢复或删除，所以这里给一个干净的、独立的清单 -->
      <template v-else-if="showDone">
        <p class="pane-done-tip">
          {{
            activeChips.length
              ? `筛出 ${completed.length} / ${completedTotal} 条 · 悬停一行可恢复 / 删除`
              : `已完成 ${completed.length} 条 · 悬停一行可恢复 / 删除`
          }}
        </p>
        <CompletedRow
          v-for="it in completed"
          :key="it.id"
          :item="it"
          @restore="restore(it)"
          @remove="softRemove(it)"
        />
        <div v-if="completed.length === 0" class="pane-empty">
          <p class="pane-empty-mark">🗂</p>
          <p class="pane-empty-title">
            {{ activeChips.length ? '没有匹配的已完成' : '还没有已完成的待办' }}
          </p>
        </div>
      </template>

      <div v-else-if="visible.length === 0 && activeChips.length > 0" class="pane-empty">
        <p class="pane-empty-mark">🔍</p>
        <p class="pane-empty-title">没搜到</p>
        <p class="pane-empty-desc">
          不是没有待办 —— 是上面这 {{ activeChips.length }} 个条件把它挡住了
        </p>
        <button type="button" class="pane-empty-btn" @click="clearAllFilters">清除全部条件</button>
      </div>

      <div v-else-if="visible.length === 0" class="pane-empty">
        <p class="pane-empty-mark">📋</p>
        <p class="pane-empty-title">
          {{ agentView ? '小刀手上暂时没有活' : '还没有待办' }}
        </p>
        <p v-if="!agentView" class="pane-empty-desc">在下面写一句，回车就记下</p>
      </div>

      <template v-else>
        <!-- 解构出 item 只是为了让我下面那段行内模板一行都不用改；
             分组标题跟着每组第一行走（见 renderRows 的说明） -->
        <div v-for="{ item: it, header, headerTone } in renderRows" :key="it.id" class="pane-item">
          <p v-if="header" class="pane-group-tip pane-due-tip" :class="`due-${headerTone}`">
            {{ header }}
          </p>
          <EditRow
            v-if="editingId === it.id"
            :item="it"
            @save="saveEdit(it, $event)"
            @cancel="editingId = null"
          />
          <template v-else>
            <ReviewRow
              v-if="agentView && it.status === 'waiting'"
              :item="it"
              :lib-root="libRoot"
              @review="(action, comment) => review(it, action, comment)"
              @done="toggleDone(it)"
              @remove="softRemove(it)"
              @edit="editingId = it.id"
              @transfer="transferringId = transferringId === it.id ? null : it.id"
              @result="showFlash($event)"
            />
            <TodoRow
              v-else
              :item="it"
              :agent-view="agentView"
              :lib-root="libRoot"
              @done="toggleDone(it)"
              @snooze="snooze(it)"
              @remove="softRemove(it)"
              @edit="editingId = it.id"
              @status="changeStatus(it, $event)"
              @transfer="transferringId = transferringId === it.id ? null : it.id"
              @result="showFlash($event)"
            />
            <TransferForm
              v-if="transferringId === it.id"
              class="pane-transfer"
              :owner="it.owner"
              :title="it.title"
              @submit="(to, note) => transfer(it, to, note)"
              @cancel="transferringId = null"
            />
          </template>
        </div>
      </template>
    </div>

    <div class="pane-foot">
      <Composer @add="addDraft" @result="showFlash($event)" />
    </div>

    <!-- 抽屉只管领域 / 权重。「显示已完成」已经挪到筛选按钮右边，那是常驻开关，
         不该藏在抽屉里点两次才能切 -->
    <FilterSheet
      :open="sheetOpen"
      :pool="pool"
      :domain="domain"
      :priority="priority"
      @update:domain="domain = $event"
      @update:priority="priority = $event"
      @close="sheetOpen = false"
    />
  </div>
</template>

<style scoped>
.pane {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 14px;
  gap: 10px;
}

.pane-head {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.pane-hint {
  margin: 0;
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.5;
  color: var(--xd-text-dim);
}

/* 提示 + 撤销。撤销只在栈非空时挂载（`v-if`，不是 disabled）——
   一个永远点不动的按钮比没有更烦人 */
.pane-toolbar {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.pane-toolbar .pane-hint {
  flex: 1;
  min-width: 0;
}

.pane-undo {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 9px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: var(--xd-card);
  color: var(--xd-text-sub);
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: calc(19px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.pane-undo:hover {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
}

.pane-flash,
.pane-error {
  margin: 0;
  padding: 4px 9px;
  border-radius: 7px;
  font-size: calc(13.2px * var(--xd-font-scale));
  animation: flash-in 0.16s var(--xd-ease);
}

.pane-flash {
  background: color-mix(in srgb, var(--xd-green) 13%, transparent);
  color: var(--xd-green);
}

.pane-error {
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
  color: var(--xd-red);
}

@keyframes flash-in {
  from {
    opacity: 0;
    transform: translateY(-3px);
  }
}

.pane-filters {
  display: flex;
  gap: 6px;
}

/* 全局搜索里的分组标题（未完成 / 已完成）—— 两组混在一起必须能一眼分开 */
.pane-group-tip {
  flex: none;
  margin: 8px 0 4px;
  font-size: calc(12.4px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.pane-search {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 0 9px;
  border-radius: 999px;
  background: var(--xd-card);
  border: 1px solid var(--xd-border);
  transition: border-color 0.15s var(--xd-ease), box-shadow 0.15s var(--xd-ease);
}

.pane-search:focus-within {
  border-color: var(--xd-accent);
  box-shadow: 0 0 0 3px var(--xd-accent-soft);
}

.pane-search-icon {
  flex: none;
  color: var(--xd-text-dim);
}

.pane-search-input {
  flex: 1;
  min-width: 0;
  padding: 6px 0;
  border: none;
  background: transparent;
  outline: none;
  font-size: calc(15px * var(--xd-font-scale));
}

.pane-search-clear {
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

.pane-filter-btn {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 0 11px;
  border-radius: 999px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.pane-filter-btn:hover {
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}

/* 图标按钮（按期限分组）：只有图标，左右内边距收到最小 ——
   360px 窗口 + 200% 字号时，这一行里每一像素都金贵 */
.pane-icon-btn {
  padding: 0 8px;
}

.pane-filter-btn.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
}

.pane-filter-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.pane-filter-btn:disabled:hover {
  border-color: var(--xd-border);
  color: var(--xd-text-sub);
}

.pane-filter-n {
  padding: 0 5px;
  border-radius: 999px;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-size: calc(12px * var(--xd-font-scale));
  font-weight: 600;
  line-height: calc(18px * var(--xd-font-scale));
}

.pane-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.pane-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: var(--xd-card);
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.pane-chip:hover {
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}

.pane-chip.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}

/* 筛选按钮右边的「已完成」勾选：跟领域/权重那些 chip 一个长相，但它常驻在这一行 */
.pane-chip-done {
  flex: none;
}

.pane-chip-n {
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12px * var(--xd-font-scale));
  opacity: 0.7;
}

.pane-active {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
}

.pane-active-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 7px;
  border-radius: 999px;
  border: 1px dashed var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(12.6px * var(--xd-font-scale));
}

.pane-active-chip:hover {
  color: var(--xd-red);
  border-color: var(--xd-red);
}

.pane-active-n {
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.pane-active-clear {
  border: none;
  background: none;
  color: var(--xd-accent);
  font-size: calc(12.6px * var(--xd-font-scale));
}

/* 列表：唯一自滚的区域 */
.pane-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 2px;
}

.pane-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.pane-transfer {
  margin-top: -2px;
}

.pane-done-tip {
  margin: 0 0 2px;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 期限分组的标题：跟搜索结果的组标题同一档字号，靠左边一条色标分轻重 ——
   「已过期」一眼就得是红的，别让用户逐字去读 */
.pane-due-tip {
  margin: 7px 0 1px;
  padding-left: 7px;
  border-left: 2px solid var(--xd-border);
}

.pane-due-tip.due-overdue {
  border-left-color: var(--xd-red);
  color: var(--xd-red);
}

.pane-due-tip.due-today {
  border-left-color: var(--xd-accent);
  color: var(--xd-accent);
}

.pane-due-tip.due-week {
  border-left-color: var(--xd-w-mid);
}

.pane-due-tip.due-later {
  border-left-color: var(--xd-w-low);
}

.pane-due-tip.due-none {
  border-left-color: var(--xd-border-soft);
}

.pane-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 26px 16px;
  text-align: center;
}

.pane-empty-mark {
  margin: 0;
  font-size: calc(26.4px * var(--xd-font-scale));
  opacity: 0.7;
}

.pane-empty-title {
  margin: 0;
  font-size: calc(15px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text-sub);
}

.pane-empty-desc {
  margin: 0;
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: 1.6;
  color: var(--xd-text-dim);
}

.pane-empty-btn {
  margin-top: 5px;
  padding: 5px 12px;
  border-radius: 999px;
  border: 1px solid var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-size: calc(13.8px * var(--xd-font-scale));
  font-weight: 600;
}

.pane-foot {
  flex: none;
}
</style>
