<script setup lang="ts">
// 数据：备份、恢复、清空、改数据目录，以及"把位置告诉 AI"。
//
// 备份目录在数据根下（`%APPDATA%\小刀工作台\待办备份\`）—— 备份是本应用自己的东西，
// 不该散在别处。
//
// **所有数据都在数据根里**（2026-09-30 起待办文件也进来了），所以这一页是唯一
// 决定"东西放哪"的地方：换目录、复制位置说明都在这，待办页不再管路径。
//
// 「改数据路径」的实现要点：那条路径**记在固定位置**（默认目录下的 `location.txt`），
// 不能存在数据根里 —— 否则成了自举循环（得先读数据根，才知道数据根在哪）。
// 换目录要重启才生效，因为 `data_root()` 在启动时解析一次就定住了。
import { inject, onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { X } from 'lucide-vue-next'
import { logToBackend, tauriApi, type BackupEntry } from '../../api/tauri'
import { askConfirm } from '../../composables/useConfirm'
import { configStore, loadConfig, saveConfig } from '../../stores/config'
import { loadTodos } from '../../stores/todo'
import { loadFocusLog } from '../../composables/useFocusLog'

const showToast = inject<(s: string) => void>('showToast', () => {})
const busy = ref(false)

// ── 数据目录 ──────────────────────────────────────────────────────────────

/** 自定义的那个路径（null = 用默认的，也就没什么可"恢复"） */
const customRoot = ref<string | null>(null)
/** 便携模式下数据跟着 exe 走，由 exe 旁边的标志文件决定 —— 这里改不了 */
const portable = ref(false)
const rootBusy = ref(false)

async function refreshRoot(): Promise<void> {
  customRoot.value = await tauriApi.dataRootCustom()
  portable.value = await tauriApi.isPortable()
}
onMounted(() => void refreshRoot())

/**
 * 换数据目录。
 * **二次确认要把后果逐条写清** —— 这是会动用户数据的操作（要复制配置/壁纸/备份），
 * 含糊其辞的确认框等于没确认。
 */
async function changeDataRoot(): Promise<void> {
  if (rootBusy.value) return
  const picked = await open({ directory: true, title: '选一个新的数据目录' })
  if (typeof picked !== 'string') return

  const ok = await askConfirm({
    title: '把数据目录改到新位置？',
    note: picked,
    facts: [
      '待办清单、配置、壁纸、备份、记账、专注记录会复制过去',
      '原目录保留、不删（后悔了还能翻回去）',
      '需要重启才生效',
    ],
    confirmText: '更改',
  })
  if (!ok) return

  rootBusy.value = true
  const r = await tauriApi.dataRootSet(picked)
  rootBusy.value = false
  if (r.startsWith('error:')) {
    // 迁移失败必须说出来：用户以为换了、实际没换，是最糟的结果
    logToBackend('error', `改数据目录失败：${r}`)
    showToast(`改不了：${r.slice(6)}`)
    return
  }
  await refreshRoot()
  logToBackend('info', r)
  showToast(r)
}

/** 切回默认目录。**不搬东西** —— 改路径时旧目录一直保留着，切回去就行 */
async function resetDataRoot(): Promise<void> {
  if (rootBusy.value) return
  // 这一步不是破坏性的（什么都不复制、也不删），所以走普通样式而不是红色
  const ok = await askConfirm({
    title: '切回默认数据目录？',
    facts: [
      '默认目录里原来那份还在（改路径时没有删过），所以这一步不复制任何东西',
      '重启后生效',
    ],
    confirmText: '切回默认',
  })
  if (!ok) return

  rootBusy.value = true
  const r = await tauriApi.dataRootReset()
  rootBusy.value = false
  if (r.startsWith('error:')) {
    logToBackend('error', `恢复默认数据目录失败：${r}`)
    showToast(`恢复失败：${r.slice(6)}`)
    return
  }
  await refreshRoot()
  logToBackend('info', r)
  showToast(r)
}

/**
 * 往剪贴板写文本。两层兜底。
 *
 * 为什么不能只用 `navigator.clipboard`：它要求 **secure context**，
 * 而 Tauri 的 webview 在某些配置下不算 —— 那时这个 API 直接是 undefined。
 * 静默失败的话，用户点了"复制"却什么都没拿到，还以为已经复制好了。
 * 老式的 `execCommand('copy')` 虽然标了废弃，但各大内核都还认，正好当兜底。
 *
 * （这个函数是 2026-09-30 从待办页跟着「复制位置说明」一起搬过来的）
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
 * 复制一段「数据位置说明」。
 *
 * **这段文字是贴给 AI / 小刀 skill 的指令**：告诉它把记着的待办路径改成新的。
 * 所以只给事实（原是什么、新是什么），不写"如果你记着旧路径""请核对"这类假设句 ——
 * 核实旧记录是它自己的事，我们只负责把新值说准。
 *
 * **待办文件的完整路径必须单独列出来**：AI 脚本里写死的是 `...\待办.json`，
 * 只报数据目录的话它还得自己拼，容易拼错。
 * 旧路径不用另存一份 —— 拿旧数据根拼上 `\待办.json` 就是了。
 */
async function copyLocationNotice(): Promise<void> {
  const root = configStore.dataRoot
  const todo = `${root}\\待办.json`
  const prevRoot = configStore.cfg.notified_data_root
  const changed = prevRoot.length > 0 && prevRoot !== root

  const lines: string[] = [
    '【小刀工作台 · 数据位置说明】',
    '',
    '请把记录里的小刀待办文件路径更新为下列新路径。',
    '',
    '待办文件：',
  ]
  if (changed) {
    lines.push(`  原：${prevRoot}\\待办.json`, `  新：${todo}`)
  } else {
    lines.push(`  ${todo}`)
  }
  lines.push('', '数据目录：')
  if (changed) {
    lines.push(`  原：${prevRoot}`, `  新：${root}`)
  } else {
    lines.push(`  ${root}`)
  }
  lines.push(
    '',
    '待办文件的数据格式没有变化。',
    `生成时间：${new Date().toLocaleString('zh-CN', { hour12: false })}`,
  )

  const ok = await copyText(lines.join('\n'))
  if (!ok) {
    showToast('复制失败了，可能是系统剪贴板被占用')
    return
  }
  // 记下"这次告诉出去的是哪个目录"，下次改动才能说出"从哪变到哪"
  await saveConfig({ notified_data_root: root })
  showToast('已复制位置说明，贴给 AI / skill 就行')
}

/**
 * 打开日志目录。
 * **打不开要把路径说出来**：还没写过日志时这个目录可能根本不存在，
 * 只说"打不开"的话用户不知道该去哪儿看、也不知道它为什么不在。
 */
async function openLogs(): Promise<void> {
  const dir = `${configStore.dataRoot}\\logs`
  const r = await tauriApi.openPath(dir, 'folder')
  showToast(r === 'ok' || r === 'browser' ? `日志目录：${dir}` : `打不开：${r}（${dir}）`)
}

async function toggleBackup(): Promise<void> {
  await saveConfig({ backup_enabled: !configStore.cfg.backup_enabled })
}

async function setKeep(e: Event): Promise<void> {
  const raw = Number((e.target as HTMLInputElement).value)
  if (!Number.isFinite(raw)) return
  const keep = Math.min(60, Math.max(1, Math.round(raw)))
  await saveConfig({ backup_keep: keep })
}

async function backupNow(): Promise<void> {
  busy.value = true
  try {
    const path = await tauriApi.backupNow()
    showToast(`已备份：${path}`)
  } catch (e) {
    showToast(String(e))
  } finally {
    busy.value = false
  }
}

async function openBackupDir(): Promise<void> {
  await tauriApi.openPath(`${configStore.dataRoot}\\待办备份`, 'folder')
}

// ── 从备份恢复 ────────────────────────────────────────────────────────────

/** 恢复浮层开着没 */
const restoreOpen = ref(false)
const backupList = ref<BackupEntry[]>([])
const restoreBusy = ref(false)

/** 字节数转人话 */
function humanSize(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  if (n >= 1024) return `${Math.round(n / 1024)} KB`
  return `${n} B`
}

async function openRestore(): Promise<void> {
  restoreOpen.value = true
  // 每次打开都重新拉：用户可能刚点过"备份"，列表得是新的
  backupList.value = await tauriApi.backupEntries()
}

/**
 * 恢复。**这是本应用里破坏性最强的操作**（比清空还狠 —— 清空是你主动的，
 * 恢复是拿一份旧数据盖掉现在的），所以确认框要把三件事写清楚：
 * 用哪一份、会盖掉什么、当前这份有退路。
 */
async function doRestore(entry: BackupEntry): Promise<void> {
  // 这份备份带不带配置快照，确认框里的说法完全不同 —— 要不要动界面设置，
  // 是用户必须知情的一件事
  // 这份备份里有什么，直接决定"会动到哪些东西" —— 用户必须知情
  const inside: string[] = ['待办清单']
  if (entry.has_config) inside.push('界面设置')
  if (entry.has_wallpaper) inside.push('壁纸')
  if (entry.has_money) inside.push('记账数据')
  if (entry.has_focus) inside.push('专注记录')
  const facts: string[] = []
  if (entry.legacy) {
    facts.push('这是早先的备份：只有清单，没有设置和壁纸')
  } else {
    facts.push(`这份备份里有：${inside.join('、')}`)
    if (entry.has_config) {
      facts.push('设置会一并恢复；但清单路径、产出根、预览器位置这类"本机路径"保持你现在这份')
    }
  }
  facts.push('这份文件本应用和 AI 共用，覆盖后 AI 读到的也是这份旧内容')
  facts.push('盘上比这份备份更新的改动都会被盖掉')

  const ok = await askConfirm({
    title: '用这份备份覆盖当前清单？',
    note: `${entry.name}（${entry.mtime}）`,
    facts,
    warning: '恢复前会自动把当前这份备份一份，后悔了能翻回来。',
    confirmText: '恢复',
    danger: true,
  })
  if (!ok) return

  restoreBusy.value = true
  const result = await tauriApi.backupRestore(entry.path, entry.has_config)
  if (result.error !== undefined) {
    // 失败必须显示原因：恢复没成功而界面装作成功，是最坏的一种
    restoreBusy.value = false
    showToast(`恢复失败：${result.error}`)
    return
  }
  // 恢复完必须重载：内存里还是旧清单，不重载界面就和盘上不一致了
  await loadTodos()
  // 设置也恢复了的话，**配置同样要重拉**：后端只写了盘上的 config.json，
  // 前端内存里还是旧值 —— 不重拉的话主题、壁纸这些要重启才看得到，
  // 用户就会以为"壁纸没恢复"（实测踩过）。重拉完 applyTheme 立即把界面套上。
  if (result.config_restored === true) {
    await loadConfig()
  }
  // 专注记录同理：内存里那份是恢复前的旧账，不重读的话回顾页还显示老数字。
  // 它是"读一次就缓存住"的（只有本应用会写它），所以这里得**强制**重读
  await loadFocusLog(true)
  restoreBusy.value = false
  restoreOpen.value = false

  const parts = [`已从 ${result.from} 恢复 ${result.count} 条`, '当前那份已备份']
  if (result.config_restored === true) parts.push('设置也已恢复')
  else if (result.has_config === true) parts.push('设置没能恢复（清单已恢复）')
  if (result.wallpaper_restored === true) parts.push('壁纸也已恢复')
  if (result.money_restored === true) parts.push('记账也已恢复')
  else if (result.has_money === true) parts.push('记账没能恢复')
  if (result.focus_restored === true) parts.push('专注记录也已恢复')
  else if (result.has_focus === true) parts.push('专注记录没能恢复')
  showToast(parts.join('，'))
}

/**
 * 清空**所有数据**，回到刚装好的状态。
 *
 * 只清待办是不够的：记账、设置、壁纸都躺在同一个数据根里。
 * 想交出一份"没人用过"的软件，得一起清掉。
 *
 * 后端会**先自动备份一份再清**（备份失败就报错、什么都不清），
 * 备份目录本身不动 —— 那是唯一的后悔药。
 */
async function clearAll(): Promise<void> {
  // 原文里 `**所有数据**` 那两个星号是**当字面量显示**的（系统弹窗不认 markdown），
  // 换成浮层之后交给样式表达强调，别再往文案里写星号
  const ok = await askConfirm({
    title: '初始化，回到刚装好的状态？',
    desc: ['会清掉所有数据：'],
    facts: [
      '待办清单（一条不剩，AI 那边读到的也是空的）',
      '记账流水',
      '专注记录（番茄钟的那些记录）',
      '所有设置（主题、快捷键、提醒偏好、领域分类）',
      '壁纸',
    ],
    warning:
      '清空前会自动备份一份，备份目录本身不会被清掉 —— 后悔了能在上面「从备份恢复」里翻回来。',
    confirmText: '初始化',
    danger: true,
  })
  if (!ok) return
  busy.value = true
  try {
    const r = await tauriApi.dataReset()
    if (r.startsWith('error:')) {
      showToast(r.slice(6))
      return
    }
    showToast('已初始化，回到刚装好的状态')
    // 配置和清单都换过了，界面得跟着回到初始状态 ——
    // 不重载的话页面上还是旧数据（数据根、待办位置的显示尤其明显）
    await loadConfig()
    await loadTodos()
    // 专注记录刚被清空，内存里那份也是旧的 —— 同样强制重读，否则回顾页还挂着老数字
    await loadFocusLog(true)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <!-- 存储 -->
  <section id="sv-sec-storage" class="sv-sec">
    <h3 class="sv-sec-title">存储</h3>

    <div class="setting-row" :class="{ disabled: portable }">
      <div class="setting-info">
        <span class="setting-name">数据路径</span>
        <span class="setting-desc">
          待办清单、配置、日志、备份、壁纸、记账、专注记录**全部**都在这个目录里
        </span>
        <span v-if="portable" class="setting-soon">
          便携模式：数据跟着 exe 走，由 exe 旁边的 portable 文件决定，这里改不了
        </span>
      </div>
      <div class="sv-path">
        <span class="sv-path-text xd-select">{{ configStore.dataRoot }}</span>
        <div class="sv-path-actions">
          <button
            type="button"
            class="sv-btn"
            :disabled="rootBusy"
            @click="tauriApi.openPath(configStore.dataRoot, 'folder')"
          >
            打开
          </button>
          <button
            type="button"
            class="sv-btn"
            :disabled="rootBusy || portable"
            @click="changeDataRoot"
          >
            更改…
          </button>
          <button
            v-if="customRoot"
            type="button"
            class="sv-btn"
            :disabled="rootBusy"
            @click="resetDataRoot"
          >
            恢复默认
          </button>
        </div>
      </div>
    </div>

    <!-- 换了目录要告诉 AI：AI / skill 那边记着旧路径，它自己不会知道 -->
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">复制位置说明</span>
        <span class="setting-desc">
          把当前位置（以及从哪变过来的）复制成一段话，贴给 AI / skill，让它把记着的待办路径更新掉
        </span>
      </div>
      <button type="button" class="sv-btn" @click="copyLocationNotice">复制</button>
    </div>

    <!-- 「打开数据目录」这行删了（2026-09-25）：上面「数据路径」那行本来就带「打开」，
         两处按钮开的是同一个目录，留着纯属重复 -->
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">日志目录</span>
        <span class="setting-desc">排错看这里</span>
      </div>
      <button type="button" class="sv-btn" @click="openLogs">打开</button>
    </div>

  </section>

  <!-- 备份与恢复（磁盘上的目录名仍是「待办备份」，不动 —— 改了老备份就找不到了） -->
  <section id="sv-sec-backup" class="sv-sec">
    <h3 class="sv-sec-title">备份与恢复</h3>
    <p class="sv-sec-note">
      每天第一次启动自动存一份，存的是待办、设置、壁纸和记账。
      清单是真会丢的，其余随时能重设
    </p>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">每日备份</span>
        <span class="setting-desc">
          每天首次启动时自动备份一次，备的是改动之前的样子。一份备份里含：清单、设置、壁纸、记账、专注记录
        </span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.backup_enabled"
        :class="{ on: configStore.cfg.backup_enabled }"
        @click="toggleBackup"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">保留份数</span>
        <span class="setting-desc">超出的从最老的删</span>
      </div>
      <input
        class="sv-input"
        style="width: 84px"
        type="number"
        min="1"
        max="60"
        :value="configStore.cfg.backup_keep"
        @change="setKeep"
      />
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">立即备份</span>
      </div>
      <div style="display: flex; gap: 6px">
        <button type="button" class="sv-btn primary" :disabled="busy" @click="backupNow">备份</button>
        <button type="button" class="sv-btn" @click="openBackupDir">备份目录</button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">从备份恢复</span>
        <span class="setting-desc">
          挑一份旧的覆盖回来。会先把当前清单备份一份留条退路；备份里带设置 / 记账 / 专注记录的会一并恢复
        </span>
      </div>
      <button type="button" class="sv-btn" @click="openRestore">恢复…</button>
    </div>

  </section>

  <!-- 恢复出厂设置：回到刚装好的状态 -->
  <section id="sv-sec-danger" class="sv-sec">
    <h3 class="sv-sec-title">恢复出厂设置</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">初始化</span>
        <span class="setting-desc">
          待办、记账、专注记录、设置、壁纸全部回到刚装好的状态。会先自动备份一份
        </span>
      </div>
      <button type="button" class="sv-btn danger" :disabled="busy" @click="clearAll">初始化</button>
    </div>
  </section>

  <!-- 恢复浮层：**必须传送出去** —— 卡片 hover 有 transform，留在原地的话
       position:fixed 会改成相对卡片定位，位置跟着鼠标乱跳 -->
  <Teleport to="body">
    <template v-if="restoreOpen">
      <div class="rs-scrim" @click="restoreOpen = false" />
      <div class="rs-sheet" role="dialog" aria-label="从备份恢复">
        <div class="rs-head">
          <span class="rs-title">从备份恢复</span>
          <button type="button" class="rs-close" aria-label="关闭" @click="restoreOpen = false">
            <X :size="13" :stroke-width="2" />
          </button>
        </div>
        <p class="rs-note">
          点「恢复」时，确认框会列出这份备份里都有些什么。当前这份会先自动备份一份，后悔了能翻回来。
        </p>
        <div class="rs-list">
          <div v-for="b in backupList" :key="b.path" class="rs-row">
            <div class="rs-info">
              <span class="rs-name">{{ b.name }}</span>
              <span class="rs-meta">
                <span>{{ b.mtime }} · {{ humanSize(b.size) }}</span>
                <span v-if="b.has_config" class="rs-tag">含设置</span>
                <span v-if="b.has_wallpaper" class="rs-tag">含壁纸</span>
                <span v-if="b.has_money" class="rs-tag">含记账</span>
                <span v-if="b.has_focus" class="rs-tag">含专注</span>
                <span v-if="b.legacy" class="rs-tag dim">旧格式 · 仅清单</span>
              </span>
            </div>
            <button type="button" class="sv-btn" :disabled="restoreBusy" @click="doRestore(b)">
              恢复
            </button>
          </div>
          <p v-if="backupList.length === 0" class="rs-empty">
            还没有备份。先点上面的「备份」存一份，或者等每天第一次启动时的自动备份
          </p>
        </div>
      </div>
    </template>
  </Teleport>
</template>

<style scoped>
/* 备份内容的小标记（含设置 / 含壁纸 / 旧格式）。
   恢复会连这些东西一起换掉，得让人一眼看得出来 */
.rs-tag {
  padding: 0 5px;
  border-radius: 999px;
  border: 1px solid var(--xd-accent);
  color: var(--xd-accent);
  font-size: calc(11.4px * var(--xd-font-scale));
  line-height: calc(16px * var(--xd-font-scale));
}

/* 旧格式那个是"提醒"，不是"特性"：中性色，别跟上面两个抢注意力 */
.rs-tag.dim {
  border-color: var(--xd-border);
  color: var(--xd-text-dim);
}

.rs-scrim {
  position: fixed;
  inset: 0;
  z-index: 65;
  /* 跟 .win 一样切圆角：窗口四角是透明的，遮罩不切就会糊在窗外 */
  border-radius: var(--xd-window-radius);
  /* 别压太黑：背后那层列表得透出轮廓，模糊才有东西可糊 */
  background: rgba(0, 0, 0, 0.28);
  animation: rs-fade 0.18s ease-out;
}

@keyframes rs-fade {
  from {
    opacity: 0;
  }
}

.rs-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  z-index: 70;
  display: flex;
  flex-direction: column;
  width: min(460px, calc(100vw - 28px));
  max-height: min(560px, calc(100vh - 28px));
  padding: 14px 16px 16px;
  border: 1px solid var(--xd-border);
  border-radius: 16px;
  background: var(--xd-sheet-bg);
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  box-shadow:
    0 10px 30px rgba(0, 0, 0, 0.5),
    inset 0 1px 0 rgba(255, 255, 255, 0.1);
  animation: rs-pop 0.18s var(--xd-ease);
}

/* 关键帧里要连 translate 一起写：animation 会整个接管 transform，
   只写 scale 会把居中那部分吃掉、弹窗跑到右下角去 */
@keyframes rs-pop {
  from {
    transform: translate(-50%, -50%) scale(0.96);
    opacity: 0;
  }
}

.rs-head {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.rs-title {
  font-size: calc(15px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

.rs-close {
  display: grid;
  place-items: center;
  width: calc(22px * var(--xd-font-scale));
  height: calc(22px * var(--xd-font-scale));
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--xd-text-dim);
  cursor: pointer;
}

.rs-close:hover {
  background: var(--xd-border-soft);
  color: var(--xd-text);
}

.rs-note {
  flex: none;
  margin: 0 0 10px;
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.6;
  color: var(--xd-text-dim);
}

.rs-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.rs-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border-radius: 8px;
  background: var(--xd-card-sub);
  border: 1px solid var(--xd-border-soft);
}

.rs-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.rs-name {
  font-size: calc(13.2px * var(--xd-font-scale));
  color: var(--xd-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rs-meta {
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
  font-variant-numeric: tabular-nums;
}

.rs-empty {
  margin: 8px 4px;
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: 1.6;
  color: var(--xd-text-dim);
}
</style>
