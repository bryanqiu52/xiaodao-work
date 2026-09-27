// 记账的统计与工具：纯函数，不碰 DOM 也不碰 invoke。
//
// 金额一律以**分**为单位（整数）—— 元做浮点统计会出 0.30000000000000004 这类尾巴，
// 只有在展示的时候才换算回元。
// 原参考实现（xifofly-工作台）的汇总卡是"全量累计、没有月切换"，
// 这里的口径改成了**全部按所选月份过滤** —— 记账看的就是"这个月"。

export interface MoneyEntry {
  id: string
  title: string
  type: 'income' | 'expense'
  /** 金额，单位分（整数，恒非负） */
  amount_cents: number
  category: string
  /** YYYY-MM-DD */
  date: string
  note: string
}

/** 分类按收支分开 —— "设计收入"当支出分类、"餐饮"当收入分类都不成立 */
export const INCOME_CATEGORIES = ['设计收入', '稿费', '授权费', '奖金', '其他'] as const
export const EXPENSE_CATEGORIES = ['餐饮', '工具订阅', '交通', '购物', '娱乐', '居住', '其他'] as const

export function categoriesOf(type: 'income' | 'expense'): readonly string[] {
  return type === 'income' ? INCOME_CATEGORIES : EXPENSE_CATEGORIES
}

/** 分 → "¥1,234.56"；整元不带小数 */
export function fmtCents(cents: number): string {
  const negative = cents < 0
  const abs = Math.abs(cents)
  const yuan = Math.floor(abs / 100)
  const fen = abs % 100
  const yuanStr = yuan.toLocaleString('zh-CN')
  const body = fen === 0 ? `¥${yuanStr}` : `¥${yuanStr}.${String(fen).padStart(2, '0')}`
  return negative ? `-${body}` : body
}

/** "YYYY-MM-DD" → "YYYY-MM" */
export function monthKeyOf(date: string): string {
  return date.slice(0, 7)
}

/** 当前月（本地时区） */
export function currentMonth(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`
}

/** 月份加减：n 可负。跨年由 Date 处理 */
export function shiftMonth(ym: string, n: number): string {
  const [y, m] = ym.split('-').map(Number)
  const dt = new Date(y ?? 1970, (m ?? 1) - 1 + n, 1)
  return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}`
}

/** "2026-09" → "2026年9月" */
export function monthLabel(ym: string): string {
  const [y, m] = ym.split('-').map(Number)
  return `${y ?? ''}年${m ?? ''}月`
}

/** 某月的全部流水，**按日期降序**（原参考实现按录入顺序，翻账很费劲 —— 改掉） */
export function entriesOfMonth(entries: readonly MoneyEntry[], ym: string): MoneyEntry[] {
  return entries
    .filter((e) => monthKeyOf(e.date) === ym)
    .sort((a, b) => (a.date === b.date ? 0 : a.date < b.date ? 1 : -1))
}

export interface MoneySummary {
  incomeCents: number
  expenseCents: number
  balanceCents: number
}

export function summarize(entries: readonly MoneyEntry[], ym: string): MoneySummary {
  let income = 0
  let expense = 0
  for (const e of entries) {
    if (monthKeyOf(e.date) !== ym) continue
    if (e.type === 'income') income += e.amount_cents
    else expense += e.amount_cents
  }
  return { incomeCents: income, expenseCents: expense, balanceCents: income - expense }
}

export interface CategorySlice {
  category: string
  cents: number
}

/**
 * 某月某类型的分类小计，降序。
 * 条形图宽度用「相对最大分类」归一化（而不是相对总额）——
 * 首位永远顶满，次位的比例关系一眼可比。
 */
export function byCategory(
  entries: readonly MoneyEntry[],
  ym: string,
  type: 'income' | 'expense',
): CategorySlice[] {
  const map = new Map<string, number>()
  for (const e of entries) {
    if (e.type !== type || monthKeyOf(e.date) !== ym) continue
    map.set(e.category, (map.get(e.category) ?? 0) + e.amount_cents)
  }
  return [...map.entries()]
    .map(([category, cents]) => ({ category, cents }))
    .sort((a, b) => b.cents - a.cents)
}

/** 条形宽度百分比（相对最大值） */
export function sliceWidth(slice: CategorySlice, slices: readonly CategorySlice[]): number {
  const max = slices[0]?.cents ?? 0
  if (max <= 0) return 0
  return Math.round((slice.cents / max) * 100)
}

/** "YYYY-MM-DD" → "9月24日 周四"（流水分组头用） */
export function dayLabel(date: string): string {
  const [y, m, d] = date.split('-').map(Number)
  const dt = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1)
  const week = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
  return `${m ?? ''}月${d ?? ''}日 ${week[dt.getDay()] ?? ''}`
}

/** 今天的 YYYY-MM-DD（新流水默认日期） */
export function todayIso(): string {
  const d = new Date()
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  return `${d.getFullYear()}-${mm}-${dd}`
}

// ── 月度对比 ──────────────────────────────────────────────────────────────

export interface MonthCompare {
  /** 上月**有没有流水**。没有的话界面要明说"上月没有记录"，不拿一堆零做对比 */
  hasPrev: boolean
  prev: MoneySummary
  /** 本月 − 上月。正数 = 比上月多 */
  incomeDiff: number
  expenseDiff: number
  balanceDiff: number
}

/**
 * 本月与上月的收支差。
 *
 * **两个字段算的是两回事**：`incomeDiff` 只是"本月减上月"的差值，
 * 而"这算好还是坏"完全看类型 —— 支出涨是坏、收入涨是好。
 * 所以好坏判断放在界面层，core 只给数字（免得把业务话术埋进纯函数里）。
 */
export function compareWithPrev(entries: readonly MoneyEntry[], ym: string): MonthCompare {
  const prevYm = shiftMonth(ym, -1)
  const prev = summarize(entries, prevYm)
  const now = summarize(entries, ym)
  return {
    hasPrev: entries.some((e) => monthKeyOf(e.date) === prevYm),
    prev,
    incomeDiff: now.incomeCents - prev.incomeCents,
    expenseDiff: now.expenseCents - prev.expenseCents,
    balanceDiff: now.balanceCents - prev.balanceCents,
  }
}

// ── 搜索 ──────────────────────────────────────────────────────────────────

/** 命中标题 / 分类 / 备注（本地、不区分大小写）。空关键词一律全收 */
export function matchesMoneyKeyword(entry: MoneyEntry, keyword: string): boolean {
  const needle = keyword.trim().toLowerCase()
  if (needle === '') return true
  return (
    entry.title.toLowerCase().includes(needle) ||
    entry.category.toLowerCase().includes(needle) ||
    (entry.note ?? '').toLowerCase().includes(needle)
  )
}

/** 筛一个已经排好序的列表，保持原顺序（不重新排 —— 搜索不该把时间序打乱） */
export function filterEntries(entries: readonly MoneyEntry[], keyword: string): MoneyEntry[] {
  if (keyword.trim() === '') return [...entries]
  return entries.filter((e) => matchesMoneyKeyword(e, keyword))
}
