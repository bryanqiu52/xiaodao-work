<script setup lang="ts">
// 记账主界面。
//
// 数据全量在前端内存（个人记账量级很小），增删改后整份写回 `数据根\记账.json`
// （Rust 原子写）。界面比原参考实现（xifofly-工作台）改进了：
//   - 有月切换，汇总全部按所选月过滤（原版是全量累计 + 只有一张"本月支出"卡）；
//   - 流水按日期分组 + 日期降序（原版按录入顺序）；
//   - 收支分类分开（原版共用一套分类）；
//   - 金额存整数分，统计不再有浮点尾巴。
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ChevronLeft, ChevronRight, Pencil, Plus, Search, Trash2, X } from 'lucide-vue-next'
import { tauriApi, type MoneyEntry } from '../api/tauri'
import { askConfirm } from '../composables/useConfirm'
import {
  byCategory,
  categoriesOf,
  compareWithPrev,
  currentMonth,
  dayLabel,
  entriesOfMonth,
  filterEntries,
  fmtCents,
  monthLabel,
  shiftMonth,
  sliceWidth,
  summarize,
  todayIso,
} from '../core/money'

const entries = ref<MoneyEntry[]>([])
const loaded = ref(false)
const ym = ref(currentMonth())
/** 当前这份数据的**内容指纹**。保存时必须带回去 —— 见 `commit()` 的注释 */
const fingerprint = ref('')
/** AI（或别处）改了记账文件之后显示的一句话 */
const externalNote = ref('')

/** 重新读一次：拿到最新流水 + 它对应的指纹 */
async function reload(): Promise<void> {
  const snap = await tauriApi.moneyRead()
  entries.value = snap.data.entries
  fingerprint.value = snap.fingerprint
  loaded.value = true
}

let unlisten: UnlistenFn | null = null

onMounted(async () => {
  await reload()
  // 记账文件被外部改动（多半是 AI 通过 skill 记了一笔）→ 跟着重载。
  // 少了这一步，AI 记完用户看到的还是旧账，会以为"AI 没记上"。
  // 比对指纹过滤自己回声那一步在 Rust 侧（watcher.rs）做完了
  unlisten = await listen('money-file-changed', async () => {
    await reload()
    externalNote.value = '记账文件刚被外部改动（可能是 AI 记了一笔），已载入最新'
    window.setTimeout(() => (externalNote.value = ''), 5000)
  })
})

onBeforeUnmount(() => {
  unlisten?.()
})

/**
 * 落盘一份新的流水。
 *
 * **带着指纹写**：记账有第二个写入方（AI 走 skill 改同一个文件）。
 * 盘上如果已经不是"我这次改动所基于的那版"，后端会**拒绝本次写入**并把盘上最新的
 * 回传 —— 这里就把最新的装回来、说一句。**绝不整份盖掉**：AI 记的那笔被静默冲掉，
 * 用户根本不会发现（账目少一笔比待办少一条难查得多）。
 */
async function commit(next: MoneyEntry[]): Promise<{ ok: boolean; msg: string }> {
  const r = await tauriApi.moneyWrite({ version: 1, entries: next }, fingerprint.value)
  if (r.err !== null) return { ok: false, msg: r.err }
  if (r.conflict) {
    // 这次改动没写进去。把盘上最新的装回来，让用户照着新的重做一次
    if (r.latest !== null) {
      entries.value = r.latest.entries
      fingerprint.value = r.fingerprint
    }
    return {
      ok: false,
      msg: '账目刚被别处改过（可能是 AI 记了一笔），这次改动没保存 —— 已载入最新，请重做一次',
    }
  }
  entries.value = next
  fingerprint.value = r.fingerprint
  return { ok: true, msg: '' }
}

const monthEntries = computed(() => entriesOfMonth(entries.value, ym.value))
const summary = computed(() => summarize(entries.value, ym.value))
const expenseSlices = computed(() => byCategory(entries.value, ym.value, 'expense'))

// ── 搜索 ──────────────────────────────────────────────────────────────────

const keyword = ref('')
const searching = computed(() => keyword.value.trim() !== '')

/**
 * 中文输入法：**拼字期间不要拿半成品去搜**。
 * 输入过程中 `input` 事件会把拼音字母（"zhizao"）也喂进来，搜出来全是空结果。
 * 等 `compositionend`（上屏）再更新关键词 —— 待办面板那边踩过同一个坑。
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
 * 列表用的流水。
 *
 * **搜索只筛列表，不动汇总卡与占比条** —— 那两张回答的是"这个月花了多少"，
 * 被搜索改掉的话，用户搜一笔小账会看到"这个月只花了 30 块"，那是错的。
 */
const listEntries = computed(() => filterEntries(monthEntries.value, keyword.value))

// ── 与上月对比 ────────────────────────────────────────────────────────────

const compare = computed(() => compareWithPrev(entries.value, ym.value))

/**
 * 涨跌该是好事还是坏事，**看类型**：支出涨是坏、收入涨是好。
 * core 只给数字，"这算好还是坏"是界面的事（业务话术别埋进纯函数）。
 */
function toneOf(diff: number, upIsGood: boolean): string {
  if (diff === 0) return ''
  return diff > 0 === upIsGood ? 'good' : 'bad'
}

/** 差额带符号：`+¥320` / `-¥80`；0 显示成"持平" */
function fmtDiff(cents: number): string {
  if (cents === 0) return '持平'
  return `${cents > 0 ? '+' : '-'}${fmtCents(Math.abs(cents))}`
}

/** 按日期分组（组内已降序） */
const groups = computed(() => {
  const out: { date: string; items: MoneyEntry[]; expenseCents: number }[] = []
  for (const e of listEntries.value) {
    const last = out[out.length - 1]
    if (last && last.date === e.date) {
      last.items.push(e)
      if (e.type === 'expense') last.expenseCents += e.amount_cents
    } else {
      out.push({
        date: e.date,
        items: [e],
        expenseCents: e.type === 'expense' ? e.amount_cents : 0,
      })
    }
  }
  return out
})

function prevMonth(): void {
  ym.value = shiftMonth(ym.value, -1)
}
function nextMonth(): void {
  ym.value = shiftMonth(ym.value, 1)
}

// ── 新增 / 编辑 ───────────────────────────────────────────────────────────

const editorOpen = ref(false)
/** 正在改的那条的 id；null = 这是新增 */
const editingId = ref<string | null>(null)
const editType = ref<'expense' | 'income'>('expense')
const editTitle = ref('')
const editAmount = ref('')
const editCategory = ref('')
const editDate = ref(todayIso())
const editNote = ref('')
const saveErr = ref('')
const saving = ref(false)

function openEditor(): void {
  editingId.value = null
  editType.value = 'expense'
  editTitle.value = ''
  editAmount.value = ''
  editCategory.value = ''
  editDate.value = todayIso()
  editNote.value = ''
  saveErr.value = ''
  editorOpen.value = true
}

/** 改已有的一笔：原样填回表单，id 留着 —— 保存时按 id 原位替换 */
function openEdit(e: MoneyEntry): void {
  editingId.value = e.id
  editType.value = e.type
  editTitle.value = e.title
  editAmount.value = (e.amount_cents / 100).toFixed(2)
  editCategory.value = e.category
  editDate.value = e.date
  editNote.value = e.note
  saveErr.value = ''
  editorOpen.value = true
}

function setType(t: 'expense' | 'income'): void {
  editType.value = t
  editCategory.value = ''
}

/**
 * 保存（新增或改同一张表单）。**分是整数**：输入元 → ×100 取整，
 * 浮点尾巴在入账前就被掐掉。全量写回；失败把原因显示在表单里（这是数据操作，不许静默）。
 */
async function save(): Promise<void> {
  if (saving.value) return
  const amount = Number(editAmount.value)
  if (!Number.isFinite(amount) || amount <= 0) {
    saveErr.value = '金额要大于 0'
    return
  }
  if (editTitle.value.trim() === '') {
    saveErr.value = '写个项目名，回头翻账认得出来'
    return
  }
  const cats = categoriesOf(editType.value)
  const fields = {
    title: editTitle.value.trim(),
    type: editType.value,
    amount_cents: Math.round(amount * 100),
    category: editCategory.value || cats[0] || '其他',
    date: editDate.value || todayIso(),
    note: editNote.value.trim(),
  }
  const id = editingId.value
  // 改：**原位替换**，不挪位置 —— 改完日期也不该从列表里跳走
  const next: MoneyEntry[] =
    id === null
      ? [{ id: crypto.randomUUID(), ...fields }, ...entries.value]
      : entries.value.map((x) => (x.id === id ? { ...x, ...fields } : x))
  saving.value = true
  const r = await commit(next)
  saving.value = false
  if (!r.ok) {
    saveErr.value = r.msg
    return
  }
  // 写的是哪个月就看到哪个月：补历史账不用手动翻回去
  ym.value = fields.date.slice(0, 7)
  editorOpen.value = false
}

async function remove(e: MoneyEntry): Promise<void> {
  // 走应用内的确认浮层（原来是系统原生框，跟这套界面完全是两个世界）
  const ok = await askConfirm({
    title: '删掉这笔流水？',
    note: `${e.title}　${e.type === 'income' ? '+' : '-'}${fmtCents(e.amount_cents)}`,
    warning: '此操作不可撤销。',
    confirmText: '删除',
    danger: true,
  })
  if (!ok) return
  const rest = entries.value.filter((x) => x.id !== e.id)
  const r = await commit(rest)
  if (!r.ok) {
    // 删除失败必须说：用户以为删了，下次打开还在
    deleteErr.value = r.msg
    window.setTimeout(() => (deleteErr.value = ''), 6000)
  }
}

/** 删除失败的提示（与编辑表单里的分开，各显各的地方） */
const deleteErr = ref('')
</script>

<template>
  <div class="money">
    <div class="money-head">
      <div class="money-month">
        <button type="button" class="money-nav" aria-label="上个月" @click="prevMonth">
          <ChevronLeft :size="14" :stroke-width="2.2" />
        </button>
        <span class="money-month-label">{{ monthLabel(ym) }}</span>
        <button type="button" class="money-nav" aria-label="下个月" @click="nextMonth">
          <ChevronRight :size="14" :stroke-width="2.2" />
        </button>
      </div>
      <button type="button" class="money-add" @click="openEditor">
        <Plus :size="14" :stroke-width="2.4" />
        记一笔
      </button>
    </div>

    <!-- 搜索：只筛下面的流水列表，上面那几张卡与占比条不受影响 -->
    <div class="money-search">
      <Search :size="12" :stroke-width="2" class="money-search-icon" />
      <!-- 不用 v-model：中文输入法拼字期间会把拼音字母喂进来（见 onSearchInput 的说明） -->
      <input
        :value="keyword"
        class="money-search-input"
        type="text"
        placeholder="搜项目 / 分类 / 备注"
        spellcheck="false"
        @input="onSearchInput"
        @compositionstart="composing = true"
        @compositionend="onCompositionEnd"
        @keydown.esc="keyword = ''"
      />
      <button
        v-if="keyword"
        type="button"
        class="money-search-clear"
        aria-label="清空搜索"
        @click="keyword = ''"
      >
        <X :size="11" :stroke-width="2.4" />
      </button>
    </div>

    <!-- 外部改动（AI 记了一笔）的提示。要说出来，否则用户看到数字变了会以为是软件抽风 -->
    <p v-if="externalNote !== ''" class="money-note">{{ externalNote }}</p>

    <!-- 汇总三卡：全部按所选月过滤 -->
    <div class="money-cards">
      <div class="mcard">
        <span class="mcard-label">收入</span>
        <span class="mcard-value in">{{ fmtCents(summary.incomeCents) }}</span>
      </div>
      <div class="mcard">
        <span class="mcard-label">支出</span>
        <span class="mcard-value out">{{ fmtCents(summary.expenseCents) }}</span>
      </div>
      <div class="mcard">
        <span class="mcard-label">结余</span>
        <span class="mcard-value" :class="{ out: summary.balanceCents < 0 }">
          {{ fmtCents(summary.balanceCents) }}
        </span>
      </div>
    </div>

    <!-- 与上月对比：**没有对比就没有感觉**，光看本月数字不知道是不是花多了。
         上月一条记录都没有时明说，不拿一堆零做对比 —— 那会得出"暴涨"的假结论 -->
    <div class="money-cmp">
      <template v-if="compare.hasPrev">
        <span class="mcmp">
          支出比上月
          <b :class="toneOf(compare.expenseDiff, false)">{{ fmtDiff(compare.expenseDiff) }}</b>
        </span>
        <span class="mcmp">
          收入比上月
          <b :class="toneOf(compare.incomeDiff, true)">{{ fmtDiff(compare.incomeDiff) }}</b>
        </span>
        <span class="mcmp">
          结余
          <b :class="toneOf(compare.balanceDiff, true)">{{ fmtDiff(compare.balanceDiff) }}</b>
        </span>
      </template>
      <span v-else class="mcmp-dim">
        {{ monthLabel(shiftMonth(ym, -1)) }}没有记录，这个月没法对比
      </span>
    </div>

    <!-- 分类占比（支出）：相对最大分类归一化，首位顶满、次位可比 -->
    <div v-if="expenseSlices.length > 0" class="money-cats">
      <div v-for="s in expenseSlices.slice(0, 5)" :key="s.category" class="mcat">
        <span class="mcat-name">{{ s.category }}</span>
        <span class="mcat-bar">
          <i :style="{ width: `${sliceWidth(s, expenseSlices)}%` }" />
        </span>
        <span class="mcat-val">{{ fmtCents(s.cents) }}</span>
      </div>
    </div>

    <p v-if="deleteErr !== ''" class="money-err">{{ deleteErr }}</p>

    <!-- 流水：按日期分组 -->
    <div class="money-list">
      <p v-if="searching" class="money-hits">
        搜「{{ keyword.trim() }}」· {{ listEntries.length }} 笔（本月共 {{ monthEntries.length }} 笔）
      </p>
      <template v-if="groups.length > 0">
        <div v-for="g in groups" :key="g.date" class="mgroup">
          <div class="mgroup-head">
            <span>{{ dayLabel(g.date) }}</span>
            <span v-if="g.expenseCents > 0" class="mgroup-sum">支 {{ fmtCents(g.expenseCents) }}</span>
          </div>
          <div v-for="e in g.items" :key="e.id" class="mrow">
            <span class="mrow-main">
              <span class="mrow-title">{{ e.title }}</span>
              <span class="mrow-sub">
                {{ e.category }}<template v-if="e.note"> · {{ e.note }}</template>
              </span>
            </span>
            <button
              type="button"
              class="mrow-act"
              :aria-label="`编辑 ${e.title}`"
              :title="`编辑「${e.title}」`"
              @click="openEdit(e)"
            >
              <Pencil :size="12" :stroke-width="2" />
            </button>
            <button
              type="button"
              class="mrow-act del"
              :aria-label="`删除 ${e.title}`"
              @click="remove(e)"
            >
              <Trash2 :size="12" :stroke-width="2" />
            </button>
            <span class="mrow-amount" :class="e.type">{{ e.type === 'income' ? '+' : '-' }}{{ fmtCents(e.amount_cents) }}</span>
          </div>
        </div>
      </template>

      <div v-else-if="loaded" class="money-empty">
        <p class="money-empty-title">
          {{ searching ? `没搜到「${keyword.trim()}」` : `${monthLabel(ym)}还没有记录` }}
        </p>
        <p class="money-empty-desc">
          {{
            searching
              ? '换个词试试 —— 搜的是项目名 / 分类 / 备注'
              : '点右上角「记一笔」添加第一条'
          }}
        </p>
        <button v-if="searching" type="button" class="money-empty-btn" @click="keyword = ''">
          清空搜索
        </button>
      </div>
    </div>

    <!-- 新增浮层：传送出去（卡片 hover 有 transform，fixed 会改基准） -->
    <Teleport to="body">
      <template v-if="editorOpen">
        <div class="me-scrim" @click="editorOpen = false" />
        <div class="me-sheet" role="dialog" :aria-label="editingId ? '改一笔' : '记一笔'">
          <div class="me-head">
            <span class="me-title">{{ editingId ? '改一笔' : '记一笔' }}</span>
            <button type="button" class="me-close" aria-label="关闭" @click="editorOpen = false">
              <X :size="13" :stroke-width="2" />
            </button>
          </div>

          <div class="me-seg" role="group" aria-label="类型">
            <button
              type="button"
              class="me-seg-btn"
              :class="{ on: editType === 'expense' }"
              @click="setType('expense')"
            >
              支出
            </button>
            <button
              type="button"
              class="me-seg-btn"
              :class="{ on: editType === 'income' }"
              @click="setType('income')"
            >
              收入
            </button>
          </div>

          <div class="me-grid">
            <label class="me-field">
              <span class="me-label">项目</span>
              <input
                v-model="editTitle"
                type="text"
                :placeholder="editType === 'expense' ? '午餐 / 工具订阅' : '设计款 / 稿费'"
                @keydown.enter="save"
              />
            </label>
            <label class="me-field">
              <span class="me-label">金额（元）</span>
              <input
                v-model="editAmount"
                type="number"
                min="0"
                step="0.01"
                placeholder="30.00"
                @keydown.enter="save"
              />
            </label>
          </div>

          <div class="me-field">
            <span class="me-label">分类</span>
            <div class="me-cats">
              <button
                v-for="c in categoriesOf(editType)"
                :key="c"
                type="button"
                class="gh-chip"
                :class="{ on: editCategory === c }"
                @click="editCategory = editCategory === c ? '' : c"
              >
                {{ c }}
              </button>
            </div>
          </div>

          <div class="me-grid">
            <label class="me-field">
              <span class="me-label">日期</span>
              <input v-model="editDate" type="date" />
            </label>
            <label class="me-field">
              <span class="me-label">备注（可选）</span>
              <input v-model="editNote" type="text" placeholder="一句话补充" @keydown.enter="save" />
            </label>
          </div>

          <p v-if="saveErr !== ''" class="me-err">{{ saveErr }}</p>

          <button type="button" class="me-save" :disabled="saving" @click="save">
            {{ saving ? '保存中…' : '保存' }}
          </button>
        </div>
      </template>
    </Teleport>
  </div>
</template>

<style scoped>
.money {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 10px 12px 12px;
  gap: 10px;
}

.money-head {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.money-month {
  display: flex;
  align-items: center;
  gap: 6px;
}

.money-nav {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border-radius: 8px;
  border: 1px solid var(--xd-border);
  background: transparent;
  color: var(--xd-text-dim);
  cursor: pointer;
}

.money-nav:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.money-month-label {
  font-size: calc(14.6px * var(--xd-font-scale));
  font-weight: 600;
  min-width: 82px;
  text-align: center;
}

.money-add {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-radius: 999px;
  border: none;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-size: calc(12.8px * var(--xd-font-scale));
  font-weight: 600;
  cursor: pointer;
}

.money-add:hover {
  background: var(--xd-accent-hover);
}

/* 外部改动的提示条 */
.money-note {
  flex: none;
  margin: 6px 0 0;
  padding: 5px 9px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--xd-accent) 12%, transparent);
  font-size: calc(12.4px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 搜索框：跟待办面板那个同一个长相（胶囊 + 聚焦时强调色描边） */
.money-search {
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

.money-search:focus-within {
  border-color: var(--xd-accent);
  box-shadow: 0 0 0 3px var(--xd-accent-soft);
}

.money-search-icon {
  flex: none;
  color: var(--xd-text-dim);
}

.money-search-input {
  flex: 1;
  min-width: 0;
  padding: 5px 0;
  border: none;
  background: transparent;
  outline: none;
  color: var(--xd-text);
  font-size: calc(13.2px * var(--xd-font-scale));
}

.money-search-clear {
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

.money-cards {
  flex: none;
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

/* 与上月对比：一行小字紧跟汇总卡。涨跌用功能色而不是强调色 ——
   强调色在这套界面里是"可点/选中"，不该被拿来表达"好/坏" */
.money-cmp {
  flex: none;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 12px;
  margin-top: -2px;
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.mcmp {
  display: inline-flex;
  align-items: baseline;
  gap: 4px;
}

.mcmp b {
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  color: var(--xd-text-sub);
}

.mcmp b.good {
  color: var(--xd-green);
}

.mcmp b.bad {
  color: var(--xd-red);
}

.mcmp-dim {
  color: var(--xd-text-dim);
}

.money-hits {
  flex: none;
  margin: 0 0 2px;
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.mcard {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 9px 12px;
  border-radius: var(--xd-radius);
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
}

.mcard-label {
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.mcard-value {
  font-size: calc(14.6px * var(--xd-font-scale));
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--xd-text);
}

.mcard-value.in {
  color: var(--xd-green);
}

.mcard-value.out {
  color: var(--xd-red);
}

.money-cats {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 10px 12px;
  border-radius: var(--xd-radius);
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
}

.mcat {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mcat-name {
  flex: none;
  width: 62px;
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mcat-bar {
  flex: 1;
  height: 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--xd-text) 8%, transparent);
  overflow: hidden;
}

.mcat-bar i {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: color-mix(in srgb, var(--xd-red) 65%, transparent);
}

.mcat-val {
  flex: none;
  min-width: 64px;
  text-align: right;
  font-size: calc(11.8px * var(--xd-font-scale));
  font-variant-numeric: tabular-nums;
  color: var(--xd-text-dim);
}

.money-err {
  flex: none;
  margin: 0;
  padding: 5px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--xd-red) 14%, transparent);
  color: var(--xd-red);
  font-size: calc(12.2px * var(--xd-font-scale));
}

.money-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-right: 2px;
}

/* 组内的行距：一笔一笔要能分开看，连成一片就分不清哪行是哪笔 */
.mgroup {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mgroup-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 2px 2px;
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.mgroup-sum {
  font-variant-numeric: tabular-nums;
}

.mrow {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--xd-radius);
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
}

.mrow-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.mrow-title {
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mrow-sub {
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 改 / 删两个按钮默认藏起来，悬停整行才出现 —— 不占地方也不误点 */
.mrow-act {
  flex: none;
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--xd-text-dim);
  cursor: pointer;
  opacity: 0;
  transition:
    opacity 0.15s,
    color 0.15s,
    background 0.15s;
}

.mrow:hover .mrow-act,
.mrow:focus-within .mrow-act {
  opacity: 1;
}

.mrow-act:hover {
  color: var(--xd-accent);
  background: var(--xd-accent-soft);
}

/* 删除是唯一不可逆的那个，单独走红色 —— 跟"改"一眼分得开 */
.mrow-act.del:hover {
  color: var(--xd-red);
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
}

.mrow-amount {
  flex: none;
  font-size: calc(13.4px * var(--xd-font-scale));
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--xd-red);
}

.mrow-amount.income {
  color: var(--xd-green);
}

.money-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  color: var(--xd-text-dim);
}

.money-empty-title {
  margin: 0;
  font-size: calc(14px * var(--xd-font-scale));
  color: var(--xd-text);
  font-weight: 600;
}

.money-empty-desc {
  margin: 0;
  font-size: calc(12.2px * var(--xd-font-scale));
}

.money-empty-btn {
  margin-top: 5px;
  padding: 5px 12px;
  border-radius: 999px;
  border: 1px solid var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-size: calc(12.8px * var(--xd-font-scale));
  font-weight: 600;
}

/* ── 新增浮层 ─────────────────────────────────────────────────────────── */

.me-scrim {
  position: fixed;
  inset: 0;
  z-index: 65;
  /* 窗口四角是透明的（圆角由 .win 裁出来），遮罩要切同样的圆角，不然糊在窗外 */
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.4);
}

.me-sheet {
  position: fixed;
  z-index: 66;
  left: 50%;
  top: 50%;
  translate: -50% -50%;
  width: min(360px, calc(100vw - 40px));
  max-height: calc(100vh - 60px);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  border-radius: var(--xd-radius-lg);
  /* 浮层必须"实"：这是盖在内容上要人看清楚再填的表单 —— 跟 FilterSheet 同一口径 */
  background: var(--xd-sheet-bg);
  /* 同上：与其它浮层共用一组变量 */
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
}

.me-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.me-title {
  font-size: calc(15px * var(--xd-font-scale));
  font-weight: 600;
}

.me-close {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--xd-text-dim);
  cursor: pointer;
}

.me-close:hover {
  color: var(--xd-text);
  background: color-mix(in srgb, var(--xd-text) 8%, transparent);
}

.me-seg {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
  padding: 3px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--xd-text) 6%, transparent);
}

.me-seg-btn {
  padding: 6px 0;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(12.8px * var(--xd-font-scale));
  cursor: pointer;
}

.me-seg-btn.on {
  background: var(--xd-card);
  color: var(--xd-text);
  font-weight: 600;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
}

.me-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.me-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.me-label {
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.me-field input {
  padding: 7px 10px;
  border-radius: 9px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
  color: var(--xd-text);
  font-size: calc(13px * var(--xd-font-scale));
  outline: none;
}

.me-field input:focus {
  border-color: var(--xd-accent);
}

.me-cats {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.me-err {
  margin: 0;
  font-size: calc(12.2px * var(--xd-font-scale));
  color: var(--xd-red);
}

.me-save {
  padding: 9px 0;
  border: none;
  border-radius: 10px;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-size: calc(13.6px * var(--xd-font-scale));
  font-weight: 600;
  cursor: pointer;
}

.me-save:hover {
  background: var(--xd-accent-hover);
}

.me-save:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* 分类 chip 复用热榜的样式（scoped 下复制一份，视觉一致） */
.gh-chip {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid var(--xd-border);
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(12.4px * var(--xd-font-scale));
  cursor: pointer;
  transition:
    border-color 0.15s,
    color 0.15s,
    background 0.15s;
}

.gh-chip.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}
</style>
