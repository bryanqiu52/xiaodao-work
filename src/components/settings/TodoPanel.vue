<script setup lang="ts">
// 待办：默认视图、显示已删除、分类、提醒。
//
// 待办文件**是共用的真相源**：本应用和 AI 都读写它。
// 这点必须在界面上说清楚 —— 用户以为它只是这个应用的私有文件时，
// 就会疑惑"我改了它，AI 那边知道吗"。
//
// 「文件在哪」**不在这里**：2026-09-30 起它固定是 `数据根\待办.json`，
// 归「设置 → 数据 → 存储」管（换位置、告诉 AI 都在那）。
import { computed, inject, ref } from 'vue'
import { Trash2 } from 'lucide-vue-next'
import { configStore, saveConfig } from '../../stores/config'
import { commit, todoStore } from '../../stores/todo'
import { DOMAIN_OPTIONS } from '../../core/constants'
import { useDomains } from '../../composables/useDomains'
import { checkRemindNow, lastNotifyError } from '../../composables/useReminder'

const { label: domainName } = useDomains()

const showToast = inject<(s: string) => void>('showToast', () => {})

// ── 领域分类 ──────────────────────────────────────────────────────────────
//
// 内置那套是给大多数人准备的通用分类（工作 / 生活 / 学习 / 其他），
// 谁都能改成自己的 —— **包括删除和改名**。
// 改完界面选项、筛选、以及 AI 那边的校验都跟着变（`syncDomains` 负责同步）。

const newKey = ref('')
const newLabel = ref('')
const domainErr = ref('')

/** 没自定义过 = 用内置默认那套 */
const usingBuiltin = computed(() => configStore.cfg.domains.length === 0)
const shownDomains = computed<string[]>(() =>
  usingBuiltin.value ? [...DOMAIN_OPTIONS] : configStore.cfg.domains,
)

/** 这个分类下还有多少条待办 —— 删之前得让用户知道会牵连多少条 */
function countUsing(key: string): number {
  return todoStore.items.filter((i) => i.domain === key && i.deletedAt === null).length
}

async function addDomain(): Promise<void> {
  const key = newKey.value.trim().toLowerCase()
  const label = newLabel.value.trim()
  domainErr.value = ''
  if (key === '') {
    domainErr.value = '得给个 key（英文，写进文件里的那个值）'
    return
  }
  if (!/^[a-z][a-z0-9_-]*$/.test(key)) {
    domainErr.value = 'key 只能用小写字母 / 数字 / - / _，且字母开头'
    return
  }
  const base = usingBuiltin.value ? [...DOMAIN_OPTIONS] : [...configStore.cfg.domains]
  if (base.includes(key)) {
    // 已存在：**填了中文名就是改名**，没填才是真重复。
    // 这样"编辑默认分类"不用另做一套 UI —— 同一个表单两用
    if (label === '') {
      domainErr.value = '这个 key 已经有了（想改名的话，填上新的中文名再点添加）'
      return
    }
    await saveConfig({
      domains: base,
      domain_labels: { ...configStore.cfg.domain_labels, [key]: label },
    })
    newKey.value = ''
    newLabel.value = ''
    showToast(`已把「${key}」改名为「${label}」`)
    return
  }
  const labels = { ...configStore.cfg.domain_labels }
  if (label !== '') labels[key] = label
  await saveConfig({ domains: [...base, key], domain_labels: labels })
  newKey.value = ''
  newLabel.value = ''
  showToast(`已加分类：${label !== '' ? label : key}`)
}

async function removeDomain(key: string): Promise<void> {
  const name = domainName(key)
  const used = countUsing(key)
  // 以**当前展示的那份列表**为基准，而不是"配置里的" ——
  // 内置模式下配置是空的，照它删等于什么都不做，默认分类就永远删不掉。
  // 这里先把内置那份固化成自定义、再删掉这一项，
  // 于是"精简默认分类"这条路通了（想只留两类也行）
  const base = usingBuiltin.value ? [...DOMAIN_OPTIONS] : [...configStore.cfg.domains]
  const rest = base.filter((d) => d !== key)
  // 迁移目标要从**删完之后**的列表里挑。用 `fallbackDomain()` 取的是当前列表第一项，
  // 而"当前"还是删之前的 —— 万一要删的正好是第一项，待办就归到一个已不存在的分类上了
  const target = rest[0] ?? 'other'

  // **先迁待办，再删分类**（顺序不能反）。
  // 以前这里只删配置、不动待办，界面上却说着"它们会归到「X」"—— 说的和做的对不上：
  // 待办仍带着已经删掉的分类值，列表里照旧显示，筛选项里却找不到它了
  if (used > 0) {
    const ok = await commit((draft) => {
      for (const it of draft) {
        if (it.domain === key && it.deletedAt === null) {
          it.domain = target as typeof it.domain
          it.trail.push({
            kind: 'edit',
            at: new Date().toISOString(),
            text: `分类「${name}」被删掉了，归到「${domainName(target)}」`,
            by: '',
          })
        }
      }
    })
    // 写盘失败（多半是清单刚被别处改过）就**不再删分类**，
    // 否则分类没了、待办还在用着它，两边对不上更难查
    if (!ok) {
      showToast('清单刚被别处改过，这次没删 —— 请重试')
      return
    }
  }

  const labels = { ...configStore.cfg.domain_labels }
  delete labels[key]
  await saveConfig({ domains: rest, domain_labels: labels })
  showToast(
    used > 0 ? `已删「${name}」，${used} 条待办已归到「${domainName(target)}」` : `已删「${name}」`,
  )
}

/** 清空 = 回到内置默认那套 */
async function resetDomains(): Promise<void> {
  await saveConfig({ domains: [], domain_labels: {} })
  showToast('已恢复默认分类')
}

async function setView(v: string): Promise<void> {
  await saveConfig({ default_view: v })
}

async function toggleDeleted(): Promise<void> {
  await saveConfig({ show_deleted: !configStore.cfg.show_deleted })
}

// 卡片色（card_color_user / card_color_agent）已经删掉了：
// 那两个设置项从没接到任何渲染逻辑上，改了没反应。归属是靠权重色条 + 编号区分的，
// 不需要再多一个颜色维度。对应字段也已从 core/config.ts 和 config.rs 里移除。

// ── 到期提醒 ──────────────────────────────────────────────────────────────

const remindBusy = ref(false)

async function toggleRemind(): Promise<void> {
  await saveConfig({ remind_enabled: !configStore.cfg.remind_enabled })
}

async function setAdvance(days: number): Promise<void> {
  await saveConfig({ remind_advance_days: days })
}

/** 手动查一次：不用等那半小时的节拍，也方便确认这条链路是通的 */
async function checkNow(): Promise<void> {
  remindBusy.value = true
  const n = await checkRemindNow()
  remindBusy.value = false
  showToast(n > 0 ? `已提醒 ${n} 条` : '这会儿没有到期的（或今天已经提醒过了）')
}
</script>

<template>
  <!-- 视图 -->
  <section id="sv-sec-view" class="sv-sec">
    <h3 class="sv-sec-title">视图</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">默认视图</span>
      </div>
      <div class="sv-seg">
        <button
          type="button"
          :class="{ on: configStore.cfg.default_view === 'user' }"
          @click="setView('user')"
        >
          我的
        </button>
        <button
          type="button"
          :class="{ on: configStore.cfg.default_view === 'agent' }"
          @click="setView('agent')"
        >
          小刀的
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">显示已删除</span>
        <span class="setting-desc">
          打开后，之前「移出」的条目会重新出现在列表里（整张压暗、带「已删除」标签），
          方便翻回去找。记录一直留着，这里只管看不看得见 —— 默认关着
        </span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.show_deleted"
        :class="{ on: configStore.cfg.show_deleted }"
        @click="toggleDeleted"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>

  </section>

  <!-- 领域分类 -->
  <section id="sv-sec-domains" class="sv-sec">
    <h3 class="sv-sec-title">领域分类</h3>
    <p class="sv-sec-note">
      待办按这些分类归组。可加、可改名、可删 —— AI 也按这份归类。
    </p>

    <div class="setting-row dm-row">
      <div class="setting-info">
        <span class="setting-name">当前分类</span>
        <span class="setting-desc">
          {{
            usingBuiltin
              ? '内置默认（还没自定义过）'
              : `自定义，共 ${shownDomains.length} 类`
          }}
        </span>
      </div>

      <div class="sv-list">
        <div v-for="d in shownDomains" :key="d" class="sv-list-item">
          <span class="sv-list-text">
            {{ domainName(d) }}
            <span class="dm-key">{{ d }}</span>
            <span v-if="countUsing(d) > 0" class="dm-used">{{ countUsing(d) }} 条在用</span>
          </span>
          <!-- 默认分类**也能删**（内置那份会先被固化成自定义再删这一项） -->
          <button
            type="button"
            class="sv-list-del"
            :aria-label="`移除 ${domainName(d)}`"
            @click="removeDomain(d)"
          >
            <Trash2 :size="12" :stroke-width="2" />
          </button>
        </div>
      </div>

      <div class="sv-path-actions dm-add">
        <input v-model="newKey" class="sv-input" placeholder="英文 key，如 design" />
        <input v-model="newLabel" class="sv-input" placeholder="中文名，如 设计" />
        <button type="button" class="sv-btn" @click="addDomain">添加</button>
      </div>
      <p v-if="domainErr !== ''" class="setting-soon">{{ domainErr }}</p>

      <div class="sv-path-actions dm-add">
        <button
          v-if="!usingBuiltin"
          type="button"
          class="sv-btn"
          title="清空自定义，回到内置那套通用分类（工作 / 生活 / 学习 / 其他）"
          @click="resetDomains"
        >
          恢复默认分类
        </button>
      </div>
    </div>
  </section>

  <!-- 提醒 -->
  <section id="sv-sec-remind" class="sv-sec">
    <h3 class="sv-sec-title">提醒</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">到期提醒</span>
        <span class="setting-desc">
          待办到期时发一条 Windows 系统通知（关掉窗口也能在通知中心看到）。
          同一条同一天只提醒一次；想改时间，在卡片上点「稍后」推后 4 小时
        </span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.remind_enabled"
        :class="{ on: configStore.cfg.remind_enabled }"
        @click="toggleRemind"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>

    <!-- 下面两行都是"提醒怎么运作"的子设置：总开关关着时它们空转，
         置灰禁用，不然又是"能点但没反应" -->
    <div class="setting-row" :class="{ disabled: !configStore.cfg.remind_enabled }">
      <div class="setting-info">
        <span class="setting-name">提前几天提醒</span>
        <span class="setting-desc">提前提醒，留出准备时间</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="d in [0, 1, 2, 3]"
          :key="d"
          type="button"
          :disabled="!configStore.cfg.remind_enabled"
          :class="{ on: configStore.cfg.remind_advance_days === d }"
          @click="setAdvance(d)"
        >
          {{ d === 0 ? '当天' : `${d} 天` }}
        </button>
      </div>
    </div>

    <div class="setting-row" :class="{ disabled: !configStore.cfg.remind_enabled }">
      <div class="setting-info">
        <span class="setting-name">立即检查</span>
        <span class="setting-desc">
          不用等那半小时的节拍，马上按上面的规则查一遍（也可以用来确认提醒这条链路是通的）
        </span>
        <span v-if="lastNotifyError" class="setting-soon">
          上次通知没发出去：{{ lastNotifyError }}
        </span>
      </div>
      <button
        type="button"
        class="sv-btn"
        :disabled="remindBusy || !configStore.cfg.remind_enabled"
        @click="checkNow"
      >
        检查
      </button>
    </div>
  </section>
</template>
