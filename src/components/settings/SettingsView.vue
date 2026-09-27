<script setup lang="ts">
// 设置页外壳：两级导航（大类 + 分区）+ 按需挂载 + 设置项搜索。
//
// **为什么改成按大类挂载**：设置项只会越加越多（M2 就要加自启、备份恢复、到期提醒、
// 壁纸、透明度……）。全量挂载会让"点开设置"先白一段，而且每个面板的初始化都白跑一遍。
// 现在右侧只挂载当前大类，切大类才取对应那份代码；鼠标悬停就预取，点下去通常已在内存里。
//
// **搜索为什么走构建期索引**：别的面板不在 DOM 里，遍历 DOM 只能搜到当前这一页。
// 索引从面板模板里抽（scripts/gen-settings-index.mjs → settingsIndex.generated.ts），
// `npm run build` 前会自动刷新。
import {
  computed,
  defineAsyncComponent,
  nextTick,
  onBeforeUnmount,
  onMounted,
  provide,
  ref,
  watch,
} from 'vue'
import {
  ChevronRight,
  Database,
  FolderOpen,
  Info,
  LayoutList,
  Palette,
  Search,
  Settings,
  Timer,
  X,
} from 'lucide-vue-next'
import { SETTINGS_INDEX } from '../settingsIndex.generated'
import './shared.css'

defineEmits<{ (e: 'close'): void }>()

/**
 * 打开时直接停在哪个大类，默认「常规」。
 * 顶部标题栏那个「关于」图标会传 `about` —— 一点就到关于页，不用自己再翻一遍导航。
 */
const props = defineProps<{ initialGroup?: GroupId }>()

// ── 结构表：大类 / 分区 / 分区归属 ────────────────────────────────────────

/** 一级：大类。**一个大类 = 一个面板文件**，也是懒加载的最小单位 */
const GROUPS = [
  { id: 'general', label: '常规', icon: Settings },
  { id: 'appearance', label: '外观', icon: Palette },
  { id: 'todo', label: '待办', icon: LayoutList },
  { id: 'tools', label: '工具', icon: Timer },
  { id: 'open', label: '打开产出', icon: FolderOpen },
  { id: 'data', label: '数据', icon: Database },
  { id: 'about', label: '关于', icon: Info },
] as const
type GroupId = (typeof GROUPS)[number]['id']

/**
 * 二级：分区。每个分区对应面板里的一块 `<section id="sv-sec-<id>">`。
 *
 * **顺序必须和面板里 section 的书写顺序一致** —— 子项菜单的排列和滚动高亮都按这份表推。
 * 不一致就会出现"菜单里字体排在强调色后面、页面里却在前面"这类错位。
 * （2026-09-24 修的两处：外观的 accent/font 反了；待办的 file/view 反了，
 * 而且 domains 那一节压根没登记 —— 分类编辑在子项菜单里没有入口。）
 */
const SECTIONS = [
  // 常规
  { id: 'shortcut', label: '快捷键' },
  { id: 'window', label: '窗口' },
  { id: 'startup', label: '启动' },
  { id: 'notify', label: '通知预览' },
  // 外观
  { id: 'theme', label: '主题' },
  { id: 'font', label: '字体' },
  { id: 'accent', label: '强调色' },
  { id: 'background', label: '背景' },
  // 待办
  { id: 'file', label: '数据文件' },
  { id: 'view', label: '视图' },
  { id: 'domains', label: '分类' },
  { id: 'remind', label: '提醒' },
  // 工具
  { id: 'focus', label: '番茄闹钟' },
  // 打开产出
  { id: 'mode', label: '打开方式' },
  { id: 'roots', label: '可打开的目录' },
  // 数据
  { id: 'storage', label: '存储' },
  { id: 'backup', label: '备份与恢复' },
  { id: 'danger', label: '恢复出厂设置' },
  // 关于。**顺序必须与 AboutPanel 里 section 的书写顺序一致**：about → changelog →
  // studio → credits → privacy
  { id: 'about', label: '关于' },
  { id: 'changelog', label: '更新日志' },
  { id: 'studio', label: '开发者' },
  { id: 'credits', label: '致谢' },
  { id: 'privacy', label: '隐私说明' },
] as const
type SectionId = (typeof SECTIONS)[number]['id']

const SECTION_LABEL = Object.fromEntries(SECTIONS.map((s) => [s.id, s.label])) as Record<
  SectionId,
  string
>

/**
 * 分区 → 大类。
 * 用 `Record<SectionId, GroupId>` 约束：**漏配一个分区，TypeScript 直接报错** ——
 * 比运行时"点了没反应"强得多。
 */
const SECTION_GROUP: Record<SectionId, GroupId> = {
  shortcut: 'general',
  window: 'general',
  startup: 'general',
  notify: 'general',
  theme: 'appearance',
  accent: 'appearance',
  font: 'appearance',
  background: 'appearance',
  view: 'todo',
  file: 'todo',
  domains: 'todo',
  remind: 'todo',
  focus: 'tools',
  mode: 'open',
  roots: 'open',
  storage: 'data',
  backup: 'data',
  danger: 'data',
  about: 'about',
  changelog: 'about',
  studio: 'about',
  credits: 'about',
  privacy: 'about',
}

/** 某个大类下有哪些分区 —— 顺序从 SECTIONS 推导，免得两张表顺序对不上 */
function sectionsOf(g: GroupId): SectionId[] {
  return SECTIONS.filter((s) => SECTION_GROUP[s.id] === g).map((s) => s.id)
}

// ── 面板按需加载 ──────────────────────────────────────────────────────────

const PANEL_LOADERS = {
  general: () => import('./GeneralPanel.vue'),
  appearance: () => import('./AppearancePanel.vue'),
  todo: () => import('./TodoPanel.vue'),
  tools: () => import('./ToolsPanel.vue'),
  open: () => import('./OpenPanel.vue'),
  data: () => import('./DataPanel.vue'),
  about: () => import('./AboutPanel.vue'),
} as const

/**
 * 懒加载包装：`delay: 200` —— 本地加载通常在几十毫秒内完成，
 * 这个延迟能让它悄悄完成、不闪占位；真慢了才显示 loading。宁可慢一点也不要白屏。
 */
const lazy = (loader: () => Promise<unknown>) =>
  defineAsyncComponent({ loader: loader as () => Promise<never>, delay: 200 })

const PANELS: Record<GroupId, ReturnType<typeof defineAsyncComponent>> = {
  general: lazy(PANEL_LOADERS.general),
  appearance: lazy(PANEL_LOADERS.appearance),
  todo: lazy(PANEL_LOADERS.todo),
  tools: lazy(PANEL_LOADERS.tools),
  open: lazy(PANEL_LOADERS.open),
  data: lazy(PANEL_LOADERS.data),
  about: lazy(PANEL_LOADERS.about),
}

const currentPanel = computed(() => PANELS[activeGroup.value])

/** 鼠标移到大类就先把它那份代码取下来：点下去时通常已在内存里，切页不再等 */
function preloadPanel(id: GroupId): void {
  void PANEL_LOADERS[id]()
}

// ── 状态与导航 ────────────────────────────────────────────────────────────

const activeGroup = ref<GroupId>(props.initialGroup ?? 'general')
// 起始分区跟着大类走：从「关于」进来时高亮的是关于那一节，而不是写死的「快捷键」
const activeSection = ref<SectionId>(sectionsOf(activeGroup.value)[0] ?? 'shortcut')
const contentRef = ref<HTMLElement | null>(null)
const query = ref('')

/**
 * props 变了也要跟上：面板没有被卸载、只是换了个入口时（比如连着点两次关于图标），
 * 不 watch 的话第二次就停在原地不动了。
 */
watch(
  () => props.initialGroup,
  (g) => {
    if (g !== undefined && g !== activeGroup.value) selectGroup(g)
  },
)

/** 面板里的操作提示统一走这里 */
const toast = ref('')
let toastTimer: number | undefined
function showToast(text: string): void {
  toast.value = text
  if (toastTimer !== undefined) window.clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => {
    toast.value = ''
  }, 2400)
}
provide('showToast', showToast)

/** 切大类：右侧换成那一页，子项定位到该类的第一个分区 */
function selectGroup(id: GroupId): void {
  const first = sectionsOf(id)[0]
  lockUntil = Date.now() + 700
  if (activeGroup.value === id) {
    // 再点当前大类 = 回到这一类顶部
    if (first) activeSection.value = first
    contentRef.value?.scrollTo({ top: 0, behavior: 'smooth' })
    return
  }
  activeGroup.value = id
  if (first) activeSection.value = first
  void nextTick(() => contentRef.value?.scrollTo({ top: 0 }))
}

/**
 * 元素相对**滚动容器**顶部的位置。
 *
 * **不能用 `offsetTop`**：`.sv-content` 只是 `overflow-y: auto`，它**不是定位祖先**
 * （没有 `position`），所以 `offsetTop` 量的是到最近那个定位祖先的距离 ——
 * 跟容器内部的滚动偏移根本不是一回事。表现就是"点了子项，标题滚过头看不见"
 * （2026-09-24 修：点「快捷键」标题跑没了）。
 */
function offsetInBox(box: HTMLElement, el: HTMLElement): number {
  return el.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop
}

/**
 * 到这个时刻之前，**不让滚动事件改高亮**。
 *
 * 平滑滚动会连着触发一串 scroll，中途掠过的分区把高亮一路抢走；最糟的是**滚不到底的
 * 分区**（这类里最后一个），它永远停在"路过"的状态上 —— 点了也不亮
 * （2026-09-24 修：点「通知预览」不变绿）。
 */
let lockUntil = 0

/** 滚到某个分区。面板是懒加载的，DOM 可能还没挂上 —— 取不到就再等一轮 */
function goToSection(id: SectionId, attempt = 0): void {
  activeSection.value = id
  lockUntil = Date.now() + 700
  const box = contentRef.value
  if (!box) return
  const el = box.querySelector<HTMLElement>(`#sv-sec-${id}`)
  if (!el) {
    if (attempt < 20) window.setTimeout(() => goToSection(id, attempt + 1), 50)
    return
  }
  box.scrollTo({ top: Math.max(0, offsetInBox(box, el) - 8), behavior: 'smooth' })
}

/** 类内滚动时同步子项高亮（只遍历当前大类已挂载的分区，没挂的自然取不到） */
function onContentScroll(): void {
  const box = contentRef.value
  if (!box) return
  // 点击引起的平滑滚动期间不判定，否则高亮会被沿途的分区抢走
  if (Date.now() < lockUntil) return
  // **按实际位置排序**再判定，不依赖 SECTIONS 表里的书写顺序 ——
  // 表顺序和面板顺序一旦对不上，高亮就会乱跳。顺序错位正是这次要修的毛病，
  // 这里再兜一层，免得将来新增分区忘了登记又重演一遍
  const rows = sectionsOf(activeGroup.value)
    .map((id) => {
      const el = box.querySelector<HTMLElement>(`#sv-sec-${id}`)
      return el === null ? null : { id, top: offsetInBox(box, el) }
    })
    .filter((r): r is { id: SectionId; top: number } => r !== null)
    .sort((a, b) => a.top - b.top)
  if (rows.length === 0) return
  let current = rows[0].id
  for (const r of rows) {
    if (r.top - 60 <= box.scrollTop) current = r.id
  }
  // **已经滚到底**：最后一个分区够不到判定线（下面没内容可滚了），
  // 不特殊处理的话它永远高亮不上
  if (box.scrollTop + box.clientHeight >= box.scrollHeight - 4) {
    current = rows[rows.length - 1].id
  }
  if (current !== activeSection.value) activeSection.value = current
}

onMounted(() => contentRef.value?.addEventListener('scroll', onContentScroll))
onBeforeUnmount(() => contentRef.value?.removeEventListener('scroll', onContentScroll))

// ── 搜索（查构建期索引，不遍历 DOM）─────────────────────────────────────

interface Hit {
  section: SectionId
  title: string
  where: string
  rank: number
}

const hits = computed<Hit[]>(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return []
  const out: Hit[] = []
  for (const it of SETTINGS_INDEX) {
    const section = it.section as SectionId
    // 索引里混进未知分区（面板改了名还没重新生成索引）就跳过，别把页面搞崩
    if (!(section in SECTION_GROUP)) continue
    const group = GROUPS.find((g) => g.id === SECTION_GROUP[section])
    const where = `${group?.label ?? ''} / ${SECTION_LABEL[section]}`
    const idx = it.title.toLowerCase().indexOf(q)
    const whereHit = where.toLowerCase().includes(q)
    if (idx < 0 && whereHit === false) continue
    // 排序：标题开头命中 > 标题中间命中 > 只命中"它在哪一类"
    out.push({ section, title: it.title, where, rank: idx === 0 ? 0 : idx > 0 ? 1 : 2 })
  }
  return out.sort((a, b) => a.rank - b.rank || a.title.length - b.title.length).slice(0, 30)
})

/** 跳到某个设置项：切大类 → 等面板挂上 → 滚过去 + 闪一下 */
function jumpToSetting(section: SectionId, title: string, attempt = 0): void {
  activeGroup.value = SECTION_GROUP[section]
  activeSection.value = section
  lockUntil = Date.now() + 700
  void nextTick(() => {
    const box = contentRef.value
    if (!box) return
    const hit = Array.from(box.querySelectorAll<HTMLElement>('.setting-name')).find(
      (el) => el.textContent?.trim() === title,
    )
    // 面板可能还在加载：这一轮找不到就等下一轮（保留搜索词，用户能看到进度）
    if (!hit && attempt < 20) {
      window.setTimeout(() => jumpToSetting(section, title, attempt + 1), 50)
      return
    }
    const row = hit?.closest<HTMLElement>('.setting-row') ?? hit
    const target = row ?? box.querySelector<HTMLElement>(`#sv-sec-${section}`)
    if (target) box.scrollTo({ top: Math.max(0, offsetInBox(box, target) - 8), behavior: 'smooth' })
    if (row) {
      row.classList.add('setting-flash')
      window.setTimeout(() => row.classList.remove('setting-flash'), 1600)
    }
    query.value = ''
  })
}
</script>

<template>
  <div class="settings-view">
    <header class="sv-header">
      <h2 class="sv-title">设置</h2>
      <button type="button" class="sv-close" aria-label="返回待办" @click="$emit('close')">
        <X :size="14" :stroke-width="2" />
      </button>
    </header>

    <div class="sv-body">
      <nav class="sv-nav" aria-label="设置分类">
        <div class="sv-search">
          <Search :size="12" :stroke-width="2" style="color: var(--xd-text-dim)" />
          <input
            v-model="query"
            class="sv-search-input"
            type="text"
            placeholder="搜索设置项"
            aria-label="搜索设置项"
            @keydown.esc="query = ''"
          />
          <button
            v-if="query"
            class="sv-search-clear"
            type="button"
            aria-label="清空搜索"
            @click="query = ''"
          >
            <X :size="10" :stroke-width="2" />
          </button>
        </div>

        <!-- 有搜索词时用结果列表替换导航 -->
        <template v-if="query.trim()">
          <button
            v-for="h in hits"
            :key="`${h.section}:${h.title}`"
            type="button"
            class="sv-nav-item sv-nav-hit"
            @click="jumpToSetting(h.section, h.title)"
          >
            <span class="sv-hit-title">{{ h.title }}</span>
            <span class="sv-hit-where">{{ h.where }}</span>
          </button>
          <p v-if="hits.length === 0" class="sv-hit-empty">没有匹配的设置项</p>
        </template>

        <template v-else>
          <template v-for="g in GROUPS" :key="g.id">
            <button
              type="button"
              class="sv-nav-item sv-nav-group"
              :class="{ active: activeGroup === g.id }"
              :aria-expanded="activeGroup === g.id"
              @click="selectGroup(g.id)"
              @mouseenter="preloadPanel(g.id)"
            >
              <component :is="g.icon" :size="13" :stroke-width="2" />
              <span>{{ g.label }}</span>
              <ChevronRight class="sv-nav-caret" :size="12" :stroke-width="2.5" aria-hidden="true" />
            </button>
            <!-- 只在当前大类下展开子项 -->
            <div v-if="activeGroup === g.id" class="sv-nav-kids">
              <button
                v-for="sid in sectionsOf(g.id)"
                :key="sid"
                type="button"
                class="sv-nav-item sv-nav-sub"
                :class="{ active: activeSection === sid }"
                :aria-current="activeSection === sid ? 'true' : undefined"
                @click="goToSection(sid)"
              >
                {{ SECTION_LABEL[sid] }}
              </button>
            </div>
          </template>
        </template>
      </nav>

      <!-- 右侧只挂载当前大类：其余不渲染（这是"点开设置不再先空白"的关键） -->
      <div ref="contentRef" class="sv-content">
        <component :is="currentPanel" />
      </div>
    </div>

    <p v-if="toast" class="sv-toast">{{ toast }}</p>
  </div>
</template>

<style scoped>
.sv-toast {
  position: absolute;
  left: 50%;
  bottom: 18px;
  transform: translateX(-50%);
  margin: 0;
  padding: 6px 14px;
  border-radius: 999px;
  background: var(--xd-text);
  color: var(--xd-bg);
  font-size: calc(13.8px * var(--xd-font-scale));
  box-shadow: var(--xd-shadow-pop);
  animation: toast-in 0.18s var(--xd-ease);
}

@keyframes toast-in {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(6px);
  }
}
</style>
