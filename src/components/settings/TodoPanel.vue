<script setup lang="ts">
// 待办：文件路径、默认视图、显示已删除、提醒。
//
// 待办文件**是共用的真相源**：本应用和 AI 都读写它。
// 这点必须在界面上说清楚 —— 用户以为它只是这个应用的私有文件时，
// 就会疑惑"我改了它，AI 那边知道吗"。
import { computed, inject, ref } from 'vue'
import { Trash2 } from 'lucide-vue-next'
import { tauriApi } from '../../api/tauri'
import { configStore, saveConfig } from '../../stores/config'
import { commit, loadTodos, todoStore } from '../../stores/todo'
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

/**
 * 往剪贴板写文本。两层兜底。
 *
 * 为什么不能只用 `navigator.clipboard`：它要求 **secure context**，
 * 而 Tauri 的 webview 在某些配置下不算 —— 那时这个 API 直接是 undefined。
 * 静默失败的话，用户点了"复制"却什么都没拿到，还以为已经复制好了。
 * 老式的 `execCommand('copy')` 虽然标了废弃，但各大内核都还认，正好当兜底。
 */
async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard !== undefined && window.isSecureContext) {
      await navigator.clipboard.writeText(text)
      return true
    }
  } catch {
    /* 掉到下面的兜底 */
  }
  try {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.setAttribute('readonly', '')
    ta.style.position = 'fixed'
    ta.style.top = '-1000px'
    document.body.appendChild(ta)
    ta.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(ta)
    return ok
  } catch {
    return false
  }
}

/**
 * 生成一段「数据位置说明」，复制走。
 *
 * **用途**：AI 那边的 skill / agent 记着旧的待办路径（写在它的配置或记忆里）。
 * 你在界面里换了位置之后它并不知道，下次可能就去读写一个不存在的文件。
 * 把这段贴给它，它就知道该更新什么了。
 *
 * 所以这里输出的**不是清单内容**，而是"位置在哪、从哪变过来的"这件事本身 ——
 * 清单内容它自己会读，不需要我们抄一遍。
 *
 * 措辞上刻意做了两件事：
 *   - **把新旧都列出来**（有旧值时才说"从…变为…"），否则它没法判断是不是自己记的那份；
 *   - **明确说"格式没变"**，免得它以为数据格式也改了，去做什么兼容处理。
 */
async function copyLocationNotice(): Promise<void> {
  const f = configStore.cfg.todo_file
  const root = configStore.dataRoot
  const prevF = configStore.cfg.notified_todo_file
  const prevRoot = configStore.cfg.notified_data_root

  const lines: string[] = [
    '【小刀工作台 · 数据位置说明】',
    '',
    '如果你（AI skill / agent）的记录里存了小刀待办的文件位置，请核对并按下面这份更新。',
    '',
    '待办文件：',
  ]
  if (prevF.length > 0 && prevF !== f) {
    lines.push(`  原：${prevF}`, `  新：${f}`, '', '（路径变了）')
  } else {
    lines.push(`  ${f}`)
  }
  lines.push('', '数据目录：')
  if (prevRoot.length > 0 && prevRoot !== root) {
    lines.push(`  原：${prevRoot}`, `  新：${root}`)
  } else {
    lines.push(`  ${root}`)
  }
  lines.push(
    '  （配置、备份、日志、壁纸都在这个目录下）',
    '',
    '待办文件的**数据格式没有变化**，只是位置 —— 不需要做格式兼容处理。',
    `生成时间：${new Date().toLocaleString('zh-CN', { hour12: false })}`,
  )

  const ok = await copyText(lines.join('\n'))
  if (!ok) {
    showToast('复制失败了，可能是系统剪贴板被占用')
    return
  }
  // 记下"这次告诉出去的是哪份路径"，下次改动才能说出"从哪变到哪"
  await saveConfig({ notified_todo_file: f, notified_data_root: root })
  showToast('已复制位置说明，贴给 AI / skill 就行')
}

async function pickTodoFile(): Promise<void> {
  const picked = await tauriApi.pickFile(['json'])
  if (!picked) return
  await saveConfig({ todo_file: picked })
  await loadTodos()
  // 位置变了，AI / skill 那边并不知道 —— 主动提一句，
  // 别指望用户自己会发现"哦这里有个复制按钮"
  showToast('已切换待办文件。AI / skill 那边记着旧路径的话，用「复制位置说明」告诉它')
}

async function openTodoDir(): Promise<void> {
  const idx = Math.max(
    configStore.cfg.todo_file.lastIndexOf('\\'),
    configStore.cfg.todo_file.lastIndexOf('/'),
  )
  const dir = idx > 0 ? configStore.cfg.todo_file.slice(0, idx) : configStore.cfg.todo_file
  await tauriApi.openPath(dir, 'folder')
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
  <!-- 数据文件 -->
  <section id="sv-sec-file" class="sv-sec">
    <h3 class="sv-sec-title">数据文件</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">待办文件</span>
        <span class="setting-desc">
          这份文件<b>本应用和 AI 共用</b>：AI 直接读写它。
          在这里的改动 AI 那边看得到；反过来 AI 改了，这里也会立刻重载
        </span>
      </div>
      <div class="sv-path">
        <span class="sv-path-text xd-select">{{ configStore.cfg.todo_file }}</span>
        <div class="sv-path-actions">
          <button type="button" class="sv-btn" @click="pickTodoFile">浏览…</button>
          <button type="button" class="sv-btn" @click="openTodoDir">打开所在目录</button>
          <button type="button" class="sv-btn" @click="copyLocationNotice">复制位置说明</button>
        </div>
      </div>
      <p class="setting-desc">换了位置要告诉 AI：点「复制位置说明」贴给它。</p>
    </div>

  </section>

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
