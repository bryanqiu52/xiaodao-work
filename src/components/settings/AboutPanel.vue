<script setup lang="ts">
// 关于：这是什么 + 版本号 + 更新日志；往下是开发者、致谢、隐私说明。
//
// 「数据放在哪」不在这里 —— 那是**数据**大类的事（数据 → 存储），
// 放在关于里只会让人以为能在这儿改（其实改不了）。
// 运行环境（技术栈 / 便携还是标准 / 数据目录）也不在这儿：那是排错用的内部信息，
// 用户平时用不上，要的时候从日志里看更准。
import { inject, ref } from 'vue'
import { Check, ChevronDown, ChevronRight, Copy, ExternalLink } from 'lucide-vue-next'
import { logToBackend, tauriApi } from '../../api/tauri'
import { checkUpdate as runCheckUpdate, updateState } from '../../composables/useUpdate'
import { GITHUB_URL } from '../../core/constants'

const showToast = inject<(s: string) => void>('showToast', () => {})

// 界面上显示的版本号。package.json / tauri.conf.json / Cargo.toml 三处都必须是
// 三段 semver（安装包和 Windows 资源文件不认两段），这里跟它们保持一致
const VERSION = '1.3.0'

/** 工作室信息。**对外统一口径**，取自品牌资料里的联系方式那一节，别另起一套 */
const STUDIO = {
  name: '溪风 XIFOFLY',
  entity: '深圳市龙华区溪风设计工作室',
  slogan: '轻盈自有回响 · Lightness echoes.',
  scope: '品牌设计 / UI·UX / 包装 / 电商视觉',
  site: 'www.xifofly.com',
  siteUrl: 'https://www.xifofly.com',
  mail: 'xifofly@163.com',
}

/**
 * 更新日志：**手写维护**，发版时在最前面加一条。
 *
 * 只写"用户能感知到的变化" —— 重构、加注释、改构建脚本这些不进这里，
 * 写了就是噪音（用户翻这个是想知道"我升级会得到什么"）。
 */
const CHANGELOG: { version: string; date: string; items: string[] }[] = [
  {
    version: '1.3.0',
    date: '2026-10-10',
    items: [
      '新建的待办自动带**看得懂的编号**了（如 `cl-001`、`plan-002`）：由「领域前缀 + 序号」拼成，跟 AI 那边引用的是同一个号，跨对话找条目不费劲',
      '按领域分别排号：客户业务的第 1 条是 `cl-001`，发展规划的是 `plan-001`；该领域已有编号的接着往下排，不会撞号',
      '前缀对照：客户业务 `cl` / 公司建设 `co` / 内容创作 `ct` / 发展规划 `plan` / 小刀升级 `as` / 个人事务 `pe`',
      '已有待办的编号**一个都没动**（`imp-…` 那些老编号照旧，关联关系不受影响）',
      '自己加的领域分类仍用随机编号，不影响使用',
    ],
  },
  {
    version: '1.2.1',
    date: '2026-10-08',
    items: [
      '待办可以换排序了：默认「重要程度」，点顶栏那颗按钮切成「最近更新」—— 谁刚动过谁排最前，讨论中的变化不用再翻着找',
      '「最近更新」把你和小刀的改动都算上：改状态、编辑、评论、评审，谁动的都算',
      '排序选了会记住，下次打开还是它',
      '切到「最近更新」时，卡片上会写出这条最近是什么时候动的 —— 排到前面却看不出为什么，比不排还让人困惑',
      '脉络里的流水改成按**时间**从新到旧排：原来只是把存的顺序倒过来，AI 写的顺序不一样就会排反',
      '脉络里每条的时间单独一行、内容另起一行，长句子不再被挤成窄条（跨年的会带上年份）',
      '期限跨年也会补上年份：明年的「03-05」跟今年的不再长得一样',
      '脉络里可以**自己记一笔**了：进度不用再往正文里堆，写一句回车就进流水，自动带上时间（记完这条在「最近更新」里也会排到前面）',
      '流水每条前面加了**类型标**（评审 / 移交 / 编辑 / 手记…）：评审本来就记在流水里，不用再单独开一栏，挂个标就能从一长串里挑出来',
      '编辑里的「汇报概要」改名「一句话结论」：它管的是卡片上显示哪句，跟「记一笔」（后来发生了什么）不是一回事，原来那个名字容易混',
      '新建待办时也能写「一句话结论」了：原来只有点 ✎ 改的时候能写，那张"完整"表单反而没有',
      '状态「等你回话」改名「待回复」：原来那句是站在小刀的活上说的，同一条转到你手上就成了"你等你自己的回话"',
      '「我的」卡片上也有状态控件了：直接点着改状态，不用再点 ✎ 进表单 —— 状态统一之后它就不该是小刀视图专属的',
      '状态筛选在「我的」里也能用了（原来只有小刀视图有）：状态是通用的，「待回复」既可以是小刀在等你，也可以是你在等客户回话',
      '「关于 → 更新」里加了源码仓库地址',
      '更新日志只保留开源发布以来的版本记录（1.2.0 起），更早那几条不再显示',
    ],
  },
  {
    version: '1.2.0',
    date: '2026-09-27',
    items: [
      '支持自动更新：每天自动查一次新版本，有新版会提示你改了什么，点一下它自己下载安装',
      '更新包带签名校验，别人换掉下载链接里的安装包也装不上',
      '首次在 GitHub 开源发布（MIT 许可）',
      '内置壁纸和 md 查看器随安装包一起提供（查看器是第三方开源工具，许可原文随包附带）',
    ],
  },
]

/** 致谢：站在谁肩膀上。带链接的点了用系统浏览器打开 */
const CREDITS: { name: string; desc: string; url: string }[] = [
  { name: 'x-hub', desc: '待办面板的原型，清单结构和分类口径都从它来', url: '' },
  { name: 'MD-Preview', desc: 'md 产出的查看器，「打开产出」里可以指定用它', url: '' },
  { name: 'Tauri 2', desc: '桌面壳：窗口、托盘、快捷键、读写文件', url: 'https://tauri.app' },
  { name: 'Vue 3', desc: '界面框架', url: 'https://vuejs.org' },
  { name: 'Lucide', desc: '这套图标', url: 'https://lucide.dev' },
]

const copied = ref(false)
const changelogOpen = ref(false)

/**
 * 展开着明细的版本号。**默认只展开最新那一版。**
 *
 * 为什么不是"一展开就列全部"：更新日志只会越攒越长，全列出来一次就是十几屏，
 * 把「关于」这一页彻底撑爆（用户只是想看看这版改了什么）。
 * 折成一行之后，一个版本占一行，翻起来是"目录"而不是"正文"。
 */
const openVersions = ref<string[]>([CHANGELOG[0]?.version ?? ''])

function toggleVersion(version: string): void {
  const at = openVersions.value.indexOf(version)
  if (at >= 0) openVersions.value.splice(at, 1)
  else openVersions.value.push(version)
}

function isVersionOpen(version: string): boolean {
  return openVersions.value.includes(version)
}

/**
 * 检查更新。
 *
 * 手动点的这次**忽略"一天一次"的节流**，而且无论有没有新版都给一句话 ——
 * 按钮点了像坏了最招人烦。
 *
 * 有新版时不用在这里跳转：`updateState` 一变，常驻的 `UpdateSheet` 自己会弹出来。
 */
async function checkUpdate(): Promise<void> {
  await runCheckUpdate(true)
  if (updateState.info) showToast(`发现新版本 ${updateState.info.version}`)
  else if (updateState.manualNote) showToast(updateState.manualNote)
  else if (updateState.error) showToast(`检查失败：${updateState.error}`)
}

/**
 * 往剪贴板写文本。两层兜底。
 *
 * 为什么不能只用 `navigator.clipboard`：它要求 secure context，
 * 而 Tauri 的 webview 在某些配置下不算 —— 那时这个 API 直接不可用。
 * 静默失败的话，用户点了「复制」什么都没拿到，还以为已经复制好了。
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
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(ta)
    return ok
  } catch {
    return false
  }
}

async function copyMail(): Promise<void> {
  const ok = await copyText(STUDIO.mail)
  if (!ok) {
    showToast('复制失败了，可能是系统剪贴板被占用')
    return
  }
  copied.value = true
  window.setTimeout(() => (copied.value = false), 1500)
  showToast(`已复制 ${STUDIO.mail}`)
}

async function openSite(): Promise<void> {
  // 后端只卡协议（http / https），失败原因要说出来
  const err = await tauriApi.openUrl(STUDIO.siteUrl)
  if (err !== null) {
    logToBackend('warn', `打开官网失败：${err}`)
    showToast(`打不开官网：${err}`)
  }
}

async function openLink(url: string): Promise<void> {
  const err = await tauriApi.openUrl(url)
  if (err !== null) {
    logToBackend('warn', `打开 ${url} 失败：${err}`)
    showToast(`打不开：${err}`)
  }
}
</script>

<template>
  <section id="sv-sec-about" class="sv-sec">
    <h3 class="sv-sec-title">关于</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">小刀工作台</span>
        <span class="setting-desc">
          待办面板的独立桌面端：跟 AI 读写同一份待办文件，快捷键一按就出来，
          也能记账、看番茄闹钟
        </span>
      </div>
      <span class="sv-version">版本 {{ VERSION }}</span>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">更新</span>
        <span class="setting-desc">每天自动查一次；有新版会提示你改了什么，点一下它自己下载安装</span>
      </div>
      <button type="button" class="sv-btn" @click="checkUpdate">检查更新</button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">源码仓库</span>
        <span class="setting-desc">更新从这里发出来；想看代码、提问题也在这儿</span>
      </div>
      <button type="button" class="sv-btn link" @click="openLink(GITHUB_URL)">
        <ExternalLink :size="12" :stroke-width="2" />
        {{ GITHUB_URL.replace('https://', '') }}
      </button>
    </div>
  </section>

  <section id="sv-sec-changelog" class="sv-sec">
    <h3 class="sv-sec-title">更新日志</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">每个版本改了什么</span>
        <span class="setting-desc">当前版本 {{ CHANGELOG[0]?.version }}</span>
      </div>
      <button type="button" class="sv-btn" :class="{ on: changelogOpen }" @click="changelogOpen = !changelogOpen">
        <ChevronDown :size="12" :stroke-width="2" :class="{ 'cl-caret-open': changelogOpen }" />
        {{ changelogOpen ? '收起' : '展开' }}
      </button>
      <div v-if="changelogOpen" class="cl-list">
        <p class="cl-tip">点版本号可以展开 / 收起</p>
        <div v-for="log in CHANGELOG" :key="log.version" class="cl-item">
          <!-- 版本头是原生 button：天然键盘可达，回车/空格就能展开 -->
          <button
            type="button"
            class="cl-head"
            :aria-expanded="isVersionOpen(log.version)"
            @click="toggleVersion(log.version)"
          >
            <ChevronRight
              :size="11"
              :stroke-width="2.4"
              class="cl-caret"
              :class="{ open: isVersionOpen(log.version) }"
            />
            <span class="cl-ver">{{ log.version }}</span>
            <span class="cl-date">{{ log.date }}</span>
            <span class="cl-count">{{ log.items.length }} 条</span>
          </button>
          <ul v-if="isVersionOpen(log.version)" class="cl-items">
            <li v-for="it in log.items" :key="it">{{ it }}</li>
          </ul>
        </div>
      </div>
    </div>
  </section>

  <section id="sv-sec-studio" class="sv-sec">
    <h3 class="sv-sec-title">开发者</h3>

    <div class="setting-row">
      <div class="studio">
        <!-- 走字符串路径，跟标题栏那枚 logo 一样：Vite 自己当资源处理 -->
        <img class="studio-mark" src="../../assets/logo.png" alt="XIFOFLY" draggable="false" />
        <div class="studio-info">
          <span class="studio-name">{{ STUDIO.name }}</span>
          <span class="studio-entity">{{ STUDIO.entity }}</span>
          <span class="studio-slogan">{{ STUDIO.slogan }}</span>
          <span class="studio-scope">{{ STUDIO.scope }}</span>
        </div>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">官网</span>
        <span class="setting-desc">案例和合作方式都在上面</span>
      </div>
      <button type="button" class="sv-btn link" @click="openSite">
        <ExternalLink :size="12" :stroke-width="2" />
        {{ STUDIO.site }}
      </button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">邮箱</span>
        <span class="setting-desc">合作询价直接发这里</span>
      </div>
      <button type="button" class="sv-btn link" :class="{ on: copied }" @click="copyMail">
        <component :is="copied ? Check : Copy" :size="12" :stroke-width="2" />
        {{ copied ? '已复制' : STUDIO.mail }}
      </button>
    </div>
  </section>

  <section id="sv-sec-credits" class="sv-sec">
    <h3 class="sv-sec-title">致谢</h3>

    <div v-for="c in CREDITS" :key="c.name" class="setting-row">
      <div class="setting-info">
        <span class="setting-name">{{ c.name }}</span>
        <span class="setting-desc">{{ c.desc }}</span>
      </div>
      <button v-if="c.url !== ''" type="button" class="sv-btn link" @click="openLink(c.url)">
        <ExternalLink :size="12" :stroke-width="2" />
        {{ c.url.replace('https://', '') }}
      </button>
    </div>
  </section>

  <section id="sv-sec-privacy" class="sv-sec">
    <h3 class="sv-sec-title">隐私说明</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">数据只在本机</span>
        <span class="setting-desc">
          待办、记账、设置、壁纸全存在本机的数据目录里，不上传、不做账号
        </span>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">唯一的联网请求</span>
        <span class="setting-desc">
          GitHub 热榜：只去拉公开的榜单数据，不带任何本机信息。除此之外全程离线
        </span>
      </div>
    </div>
  </section>

</template>

<style scoped>
.sv-version {
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(13.8px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* ── 更新日志 ────────────────────────────────────────────────────────── */

/* 折起来之后它是一份"目录"（一版一行），不再是一大坨正文，所以行距收紧 */
.cl-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 2px;
}

.cl-tip {
  margin: 0 0 4px;
  padding-left: 6px;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

.cl-item {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

/* 版本头：横排一行 —— 箭头 / 版本号 / 日期 / 条数。
   做成按钮是为了能点、能键盘操作；底色默认透明，看着仍是"标题"不是"按钮" */
.cl-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 4px 6px;
  border: none;
  border-radius: 7px;
  background: transparent;
  text-align: left;
  transition: background 0.14s var(--xd-ease);
}

.cl-head:hover {
  background: var(--xd-card-sub);
}

/* 箭头在"折起来"和"展开"之间转 90° */
.cl-caret {
  flex: none;
  color: var(--xd-text-dim);
  transition: transform 0.16s var(--xd-ease);
}

.cl-caret.open {
  transform: rotate(90deg);
}

.cl-ver {
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(13.2px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-accent);
}

.cl-date {
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 条数靠右：一眼看出哪一版改动多，不用展开 */
.cl-count {
  margin-left: auto;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 小字号的列表：行距给足，不然四五条挤成一团看不下去。
   `padding-left` 用固定 25px（6 头部内边距 + 11 箭头 + 8 间距）——
   箭头本身不随字号缩放，所以缩进也不该缩放，否则放大后对不齐版本号 */
.cl-items {
  margin: 0;
  padding-left: 25px;
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.65;
  color: var(--xd-text-sub);
}

.cl-caret-open {
  transform: rotate(180deg);
}

/* ── 工作室那一块 ────────────────────────────────────────────────────── */
.studio {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.studio-mark {
  flex: none;
  width: calc(38px * var(--xd-font-scale));
  height: calc(38px * var(--xd-font-scale));
  border-radius: 9px;
  /* logo 是透明底的黑＋霓虹绿双色画：垫一层白底，深色主题下黑色笔画才看得清 */
  background: #ffffff;
  object-fit: contain;
  box-shadow: 0 0 0 1px rgba(16, 24, 40, 0.08);
}

.studio-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.studio-name {
  font-size: calc(14.4px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

.studio-entity {
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-text-sub);
}

/* 标语用强调色：这是"广告"里唯一该被看见的一行，别抢设置项的注意力 */
.studio-slogan {
  margin-top: 2px;
  font-size: calc(12.6px * var(--xd-font-scale));
  color: var(--xd-accent);
}

.studio-scope {
  font-size: calc(12px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}

/* 带图标的按钮：图标跟文字之间留一口气、垂直居中 */
.sv-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

/* 官网 / 邮箱那两个按钮带图标：排成一行并居中，图标跟文字之间留一口气 */
.sv-btn.link {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

/* 复制成功后按钮进 on 态（强调色），跟别处"点了有反馈"的口径一致 */
.sv-btn.link.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}
</style>
