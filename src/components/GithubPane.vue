<script setup lang="ts">
// GitHub 热榜主界面。
//
// 数据由 Rust 侧拉取（缓存 10 分钟），筛选变化就重新请求 ——
// 排名是按 star 排的全站热榜，本地过滤语言会得到"假排名"，所以必须重拉。
// 原参考实现在 UTC 日期上偏过一天、star 数用全量千分位 —— 这里都已修正。
import { computed, onMounted, ref } from 'vue'
import { ExternalLink, RefreshCw, Search, Star, TriangleAlert } from 'lucide-vue-next'
import { isTauri, tauriApi, type GithubRepo } from '../api/tauri'

const LANGS = [
  { key: '', label: '全部' },
  { key: 'javascript', label: 'JavaScript' },
  { key: 'typescript', label: 'TypeScript' },
  { key: 'python', label: 'Python' },
  { key: 'rust', label: 'Rust' },
  { key: 'go', label: 'Go' },
  { key: 'cpp', label: 'C++' },
  { key: 'java', label: 'Java' },
] as const

const SINCES = [
  { key: '7d', label: '近 1 周' },
  { key: '30d', label: '近 1 月' },
  { key: '90d', label: '近 3 月' },
] as const

/** GitHub 官方语言色：一眼认出语言，不用读字 */
const LANG_COLORS: Record<string, string> = {
  JavaScript: '#f1e05a',
  TypeScript: '#3178c6',
  Python: '#3572a5',
  Rust: '#dea584',
  Go: '#00add8',
  'C++': '#f34b7d',
  C: '#555555',
  Java: '#b07219',
  HTML: '#e34c26',
  CSS: '#563d7c',
  Shell: '#89e051',
  Vue: '#41b883',
  Kotlin: '#a97bff',
  'Jupyter Notebook': '#da5b0b',
  Makefile: '#427819',
  Dockerfile: '#384d54',
}

const lang = ref('')
const since = ref('30d')
const items = ref<GithubRepo[]>([])
const fetchedAt = ref('')
const loading = ref(false)
const error = ref('')
const query = ref('')
/** 区分"还没抓过"（引导去刷新）和"抓了但没结果"（换条件）—— 两种空态文案不同 */
const everFetched = ref(false)

async function fetchList(): Promise<void> {
  if (loading.value) return
  loading.value = true
  error.value = ''
  try {
    const r = await tauriApi.githubTrending(lang.value, since.value)
    items.value = r.items
    fetchedAt.value = r.fetched_at
    everFetched.value = true
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

function setLang(k: string): void {
  if (lang.value === k || loading.value) return
  lang.value = k
  void fetchList()
}

function setSince(k: string): void {
  if (since.value === k || loading.value) return
  since.value = k
  void fetchList()
}

/** 搜索框本地过滤：只过滤当前这 20 条，不重新请求 */
const shown = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return items.value
  return items.value.filter(
    (r) => r.full_name.toLowerCase().includes(q) || r.description.toLowerCase().includes(q),
  )
})

/** star 数：上万用"万"缩写（大数一眼可比），其余千分位 */
function fmtStars(n: number): string {
  if (n >= 10000) return `${(n / 10000).toFixed(1)}w`
  return n.toLocaleString('zh-CN')
}

async function openRepo(r: GithubRepo): Promise<void> {
  const err = await tauriApi.githubOpen(r.html_url)
  if (err !== null) error.value = err
}

onMounted(() => {
  // 进界面就拉（缓存命中则是秒回），不用等用户点刷新
  if (isTauri()) void fetchList()
})
</script>

<template>
  <div class="gh">
    <div class="gh-toolbar">
      <div class="gh-chips" role="group" aria-label="语言筛选">
        <button
          v-for="l in LANGS"
          :key="l.key"
          type="button"
          class="gh-chip"
          :class="{ on: lang === l.key }"
          @click="setLang(l.key)"
        >
          {{ l.label }}
        </button>
      </div>
      <div class="gh-chips" role="group" aria-label="时间范围">
        <button
          v-for="s in SINCES"
          :key="s.key"
          type="button"
          class="gh-chip"
          :class="{ on: since === s.key }"
          @click="setSince(s.key)"
        >
          {{ s.label }}
        </button>
        <button type="button" class="gh-chip gh-refresh" :disabled="loading" @click="fetchList">
          <RefreshCw :size="11" :stroke-width="2" :class="{ spin: loading }" />
          刷新
        </button>
      </div>
    </div>

    <div class="gh-search">
      <Search :size="12" :stroke-width="2" />
      <input
        v-model="query"
        type="text"
        placeholder="在结果里搜仓库名或描述"
        aria-label="过滤榜单"
        @keydown.esc="query = ''"
      />
    </div>

    <p class="gh-status" :class="{ err: error !== '' }">
      <template v-if="loading">抓取中…</template>
      <template v-else-if="error !== ''">{{ error }}</template>
      <template v-else-if="items.length > 0">
        {{ shown.length }} 条 · 更新于 {{ fetchedAt }}
      </template>
      <template v-else>还没抓取</template>
    </p>

    <div v-if="error !== '' && items.length === 0" class="gh-empty">
      <p class="gh-empty-mark"><TriangleAlert :size="22" :stroke-width="1.8" /></p>
      <p class="gh-empty-title">拉取失败了</p>
      <p class="gh-empty-desc">{{ error }}</p>
      <button type="button" class="sv-btn" @click="fetchList">重试</button>
    </div>

    <div v-else-if="shown.length > 0" class="gh-list">
      <button
        v-for="(r, i) in shown"
        :key="r.id"
        type="button"
        class="gh-row"
        :title="r.html_url"
        @click="openRepo(r)"
      >
        <span class="rank" :class="{ top: i < 3 }">{{ i + 1 }}</span>
        <span class="gh-main">
          <span class="gh-name">{{ r.full_name }}</span>
          <span class="gh-desc">{{ r.description || '（无描述）' }}</span>
          <span class="gh-meta">
            <span v-if="r.language" class="gh-lang">
              <i class="dot" :style="{ background: LANG_COLORS[r.language] ?? '#8b949e' }" />
              {{ r.language }}
            </span>
            <span class="gh-stars"><Star :size="11" :stroke-width="2" /> {{ fmtStars(r.stargazers_count) }}</span>
            <span class="gh-push">更新 {{ r.pushed_at }}</span>
          </span>
        </span>
        <ExternalLink class="gh-go" :size="13" :stroke-width="2" />
      </button>
    </div>

    <div v-else-if="everFetched" class="gh-empty">
      <p class="gh-empty-title">没有匹配的仓库</p>
      <p class="gh-empty-desc">是搜索词把它挡住了，或者换个语言 / 时间范围试试</p>
    </div>

    <div v-else class="gh-empty">
      <p class="gh-empty-title">还没有热榜数据</p>
      <p class="gh-empty-desc">点右上角「刷新」拉取 GitHub 近期最热的仓库</p>
    </div>
  </div>
</template>

<style scoped>
.gh {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 10px 12px 12px;
  gap: 8px;
}

.gh-toolbar {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.gh-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

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

.gh-chip:hover {
  color: var(--xd-text);
  border-color: var(--xd-text-dim);
}

.gh-chip.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}

.gh-chip:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.gh-refresh {
  margin-left: auto;
}

.spin {
  animation: spin 0.9s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.gh-search {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: var(--xd-radius);
  border: 1px solid var(--xd-border);
  color: var(--xd-text-dim);
}

.gh-search input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--xd-text);
  font-size: calc(12.8px * var(--xd-font-scale));
}

.gh-status {
  flex: none;
  margin: 0;
  font-size: calc(12.2px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.gh-status.err {
  color: var(--xd-red);
}

.gh-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-right: 2px;
}

.gh-row {
  flex: none;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: 100%;
  text-align: left;
  padding: 10px 12px;
  border-radius: var(--xd-radius);
  border: 1px solid var(--xd-border);
  background: var(--xd-card);
  cursor: pointer;
  transition: border-color 0.15s;
}

.gh-row:hover {
  border-color: var(--xd-accent);
}

.rank {
  flex: none;
  min-width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border-radius: 8px;
  border: 1px solid var(--xd-border);
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  font-variant-numeric: tabular-nums;
}

/* 前三名高亮：榜单的头等事就是"谁是前三" */
.rank.top {
  border-color: color-mix(in srgb, var(--xd-accent) 45%, transparent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 700;
}

.gh-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.gh-name {
  font-size: calc(13.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.gh-desc {
  font-size: calc(12.2px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.gh-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: calc(11.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.gh-lang {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex: none;
}

.gh-stars {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-variant-numeric: tabular-nums;
}

.gh-go {
  flex: none;
  color: var(--xd-text-dim);
  margin-top: 3px;
}

.gh-row:hover .gh-go {
  color: var(--xd-accent);
}

.gh-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--xd-text-dim);
  text-align: center;
  padding: 20px;
}

.gh-empty-mark {
  margin: 0;
  color: var(--xd-orange);
}

.gh-empty-title {
  margin: 0;
  font-size: calc(14.2px * var(--xd-font-scale));
  color: var(--xd-text);
  font-weight: 600;
}

.gh-empty-desc {
  margin: 0;
  font-size: calc(12.4px * var(--xd-font-scale));
  line-height: 1.5;
}
</style>
