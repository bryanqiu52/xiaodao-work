// invoke 封装。
//
// 关键点：**浏览器里也要能跑**（`npm run dev` 直接开浏览器调界面）。
// 桌面端编译依赖 C++ 工具链，本机不一定装得上，所以非 Tauri 环境一律走 mock，
// 界面照常渲染、照常可点，只是改动不落盘。

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { defaultConfig, type AppConfig } from '../core/config'
import type { TodoItem } from '../core/types'

/**
 * 是不是跑在桌面（Tauri）里。
 *
 * **只认一个全局变量会漏**：不同版本注入的全局名不完全一样，认不出来时前端会
 * 走进"浏览器兜底"分支 —— 界面照常能点、开关图标也会变，但改动一律不落盘，
 * 用户看到的就是"按了没反应"。所以这里多认几个名字。
 */
export function isTauri(): boolean {
  if (typeof window === 'undefined') return false
  const w = window as unknown as Record<string, unknown>
  return '__TAURI_INTERNALS__' in w || '__TAURI__' in w
}

/**
 * 把前端的异常写进 Rust 日志（`数据根\logs\xiaodao_work.log`）。
 *
 * **为什么需要**：桌面窗口里没法开 DevTools，`console.error` 打完就没了，
 * 事后排查"按钮按了没反应"这类静默故障连线索都没有。有了这条通道，
 * 以后报 bug 让你看一眼日志就能定位。
 */
export function logToBackend(level: 'error' | 'warn' | 'info', message: string): void {
  invoke('frontend_log', { level, message }).catch(() => {})
}

/**
 * 窗控命令的统一入口。
 *
 * **刻意不走 `isTauri()` 判断**：判断失灵时宁可真调一次拿到错误，也不要静默跳过 ——
 * 返回 null = 成功，返回字符串 = 错误文案（交给界面显示，同时写进日志）。
 */
async function winCommand(
  name: string,
  payload?: Record<string, unknown>,
): Promise<string | null> {
  try {
    await invoke(name, payload ?? {})
    return null
  } catch (e) {
    const msg = String(e)
    logToBackend('error', `窗控失败 ${name}: ${msg}`)
    return msg
  }
}

export interface TodoSnapshot {
  text: string
  fingerprint: string
  exists: boolean
}

export interface WriteResult {
  ok: boolean
  conflict: boolean
  fingerprint: string
  latest: string | null
}

// ── mock：浏览器调试用 ────────────────────────────────────────────────────

/**
 * 浏览器调试用的**示例数据**。
 *
 * **内容是虚构的，别把真实待办写进来。** 这里以前放的是作者自己的清单
 * （含客户信息、营业执照登记地址、"老板交办"这类原话），一旦代码公开，
 * 那些东西就跟着上了公网 —— 而且**不会报错、不会有提示**，只会在那儿静静躺着。
 * 私人内容属于本地数据，不属于源码。
 *
 * 这五条是挑着来的，为了把界面的各种状态都演示到：
 * 两种归属 × 五种状态 × 有无期限 × 审批留痕 × 产出文件 × 关联待办。
 */
const MOCK_ITEMS: TodoItem[] = [
  {
    id: 'demo-001',
    title: '把这周的会议记录整理成一页结论',
    detail: '示例数据：这条演示「我的」清单里一条普通的排队事项。',
    summary: '列个提纲就行',
    domain: 'work',
    owner: 'user',
    status: 'todo',
    priority: 'high',
    dueAt: '2026-10-02',
    createdAt: '2026-09-25T09:12:00.000Z',
    doneAt: null,
    deletedAt: null,
    reviews: [],
    trail: [{ kind: 'create', at: '2026-09-25T09:12:00.000Z', text: '建了这条', by: '' }],
    origin: { from: '', why: '' },
    relates: [],
    files: ['docs/示例产出.md'],
    source: '',
    link: '',
  },
  {
    id: 'demo-002',
    title: '把上面的会议记录做成汇报稿',
    detail: '示例数据：这条演示「等老板回话」，并且已经被退回重做过一次。',
    summary: '按退回意见改完了，等确认',
    domain: 'work',
    owner: 'agent',
    status: 'waiting',
    priority: 'mid',
    dueAt: '2026-09-30',
    createdAt: '2026-09-20T06:20:00.000Z',
    doneAt: null,
    deletedAt: null,
    reviews: [
      {
        action: 'rejected',
        at: '2026-09-22T10:02:00.000Z',
        comment: '只看了一条记录就下结论，重新过一遍全部',
      },
    ],
    trail: [
      { kind: 'create', at: '2026-09-20T06:20:00.000Z', text: '建了这条', by: '' },
      { kind: 'review', at: '2026-09-22T10:02:00.000Z', text: '老板退回：只看了一条就下结论', by: '' },
      { kind: 'status', at: '2026-09-23T09:00:00.000Z', text: '状态「排队」→「等你回话」', by: '' },
    ],
    origin: { from: '老板说要给客户看', why: '想把结论固定下来' },
    relates: ['demo-001'],
    files: ['docs/示例产出.md'],
    source: '',
    link: '',
  },
  {
    id: 'demo-003',
    title: '研究一下自动更新的签名该怎么管',
    detail: '示例数据：这条演示「进行中」。',
    summary: '在查 Tauri updater 的密钥怎么存',
    domain: 'study',
    owner: 'agent',
    status: 'progress',
    priority: 'high',
    dueAt: '2026-09-28',
    createdAt: '2026-09-22T02:40:00.000Z',
    doneAt: null,
    deletedAt: null,
    reviews: [],
    trail: [{ kind: 'create', at: '2026-09-22T02:40:00.000Z', text: '建了这条', by: '' }],
    origin: { from: '', why: '' },
    relates: [],
    files: [],
    source: '',
    link: '',
  },
  {
    id: 'demo-004',
    title: '把书桌收拾一下',
    detail: '示例数据：这条演示「已暂停」和「没有期限」。',
    summary: '',
    domain: 'life',
    owner: 'user',
    status: 'paused',
    priority: 'low',
    dueAt: null,
    createdAt: '2026-09-10T13:00:00.000Z',
    doneAt: null,
    deletedAt: null,
    reviews: [],
    trail: [
      { kind: 'create', at: '2026-09-10T13:00:00.000Z', text: '建了这条', by: '' },
      { kind: 'status', at: '2026-09-12T08:30:00.000Z', text: '状态「排队」→「已暂停」', by: '' },
    ],
    origin: { from: '', why: '' },
    relates: [],
    files: [],
    source: '',
    link: '',
  },
  {
    id: 'demo-005',
    title: '换了新的快捷键，试一下会不会冲突',
    detail: '示例数据：这条演示「已完成」和「老板通过」。',
    summary: '换成 Ctrl+Alt+Space 之后没再冲突',
    domain: 'other',
    owner: 'agent',
    status: 'done',
    priority: 'mid',
    dueAt: '2026-09-05',
    createdAt: '2026-09-01T02:10:00.000Z',
    doneAt: '2026-09-04T11:26:00.000Z',
    deletedAt: null,
    reviews: [{ action: 'approved', at: '2026-09-04T11:26:00.000Z', comment: '' }],
    trail: [
      { kind: 'create', at: '2026-09-01T02:10:00.000Z', text: '建了这条', by: '' },
      { kind: 'review', at: '2026-09-04T11:26:00.000Z', text: '老板通过', by: '' },
    ],
    origin: { from: '', why: '' },
    relates: [],
    files: [],
    source: '',
    link: '',
  },
]

function mockSnapshot(): TodoSnapshot {
  return {
    text: JSON.stringify(
      { version: 2, updatedAt: new Date().toISOString(), items: MOCK_ITEMS },
      null,
      2,
    ),
    fingerprint: 'mock-fingerprint',
    exists: true,
  }
}

// ── 命令 ──────────────────────────────────────────────────────────────────

export const tauriApi = {
  async todoRead(): Promise<TodoSnapshot> {
    if (!isTauri()) return mockSnapshot()
    return invoke<TodoSnapshot>('todo_read')
  },

  async todoWrite(text: string, baseFingerprint: string): Promise<WriteResult> {
    if (!isTauri()) {
      return { ok: true, conflict: false, fingerprint: 'mock-fingerprint', latest: null }
    }
    return invoke<WriteResult>('todo_write', { text, baseFingerprint })
  },

  async configGet(): Promise<AppConfig> {
    if (!isTauri()) return defaultConfig()
    return invoke<AppConfig>('config_get')
  },

  async configSet(cfg: AppConfig): Promise<AppConfig> {
    if (!isTauri()) return cfg
    return invoke<AppConfig>('config_set', { cfg })
  },

  async backupNow(): Promise<string> {
    if (!isTauri()) return '（浏览器调试模式，未真正备份）'
    return invoke<string>('backup_now')
  },

  async backupList(): Promise<string[]> {
    if (!isTauri()) return []
    return invoke<string[]>('backup_list')
  },

  async dataRootPath(): Promise<string> {
    if (!isTauri()) return '%APPDATA%\\小刀工作台'
    return invoke<string>('data_root_path')
  },

  /** 当前数据根是不是用户自定义的（null = 用默认的，也就没什么可"恢复"） */
  async dataRootCustom(): Promise<string | null> {
    if (!isTauri()) return null
    return invoke<string | null>('data_root_custom')
  },

  /**
   * 换数据根：会先把配置 / 壁纸 / 备份搬过去，**成功才记新路径**。
   * 返回一句可以直接说给用户听的话；失败回 `error:...`（跟 openPath 一个口径，不抛异常）——
   * 迁移失败的原因必须传到界面上，这是会动用户数据的操作。
   */
  async dataRootSet(path: string): Promise<string> {
    if (!isTauri()) return 'error:（浏览器调试模式改不了）'
    try {
      return await invoke<string>('data_root_set', { path })
    } catch (e) {
      return `error:${String(e)}`
    }
  },

  async dataRootReset(): Promise<string> {
    if (!isTauri()) return 'error:（浏览器调试模式改不了）'
    try {
      return await invoke<string>('data_root_reset')
    } catch (e) {
      return `error:${String(e)}`
    }
  },

  /**
   * 清空**所有数据**，回到刚装好的状态 —— 全应用唯一不可逆的操作。
   *
   * 后端会**先自动备份一份再清**，备份失败就直接报错、什么都不清；
   * 备份目录本身不动（那是唯一的后悔药）。
   * 成功回一句给人看的话，失败回 `error:<原因>`。
   */
  async dataReset(): Promise<string> {
    if (!isTauri()) return 'error:（浏览器调试模式清不了）'
    try {
      return await invoke<string>('data_reset')
    } catch (e) {
      logToBackend('error', `清空数据失败：${String(e)}`)
      return `error:${String(e)}`
    }
  },

  async todoFilePath(): Promise<string> {
    if (!isTauri()) return defaultConfig().todo_file
    return invoke<string>('todo_file_path')
  },

  /**
   * 打开产出文件。
   * `file` = 交查看器 / 系统默认程序打开；`folder` = 打开所在目录并选中。
   * 后端有白名单与"可执行文件不代跑"的兜底，这里不做判断。
   */
  async openPath(path: string, mode: 'file' | 'folder' = 'file'): Promise<string> {
    if (!isTauri()) return 'browser'
    return invoke<string>('open_path', { path, mode })
  },

  /** 弹出系统目录选择框（设置页改路径用）；返回 null 表示用户取消 */
  async pickDirectory(current?: string): Promise<string | null> {
    if (!isTauri()) return null
    return invoke<string | null>('pick_directory', { current: current ?? '' })
  },

  /** 弹出系统文件选择框 */
  async pickFile(filters?: string[]): Promise<string | null> {
    if (!isTauri()) return null
    return invoke<string | null>('pick_file', { filters: filters ?? [] })
  },

  /**
   * 改全局快捷键（会立刻重新注册）。
   * 返回 null = 成功，否则是错误文案（已区分"冲突"与"格式无效"）。
   */
  async setShortcut(v: string): Promise<string | null> {
    if (!isTauri()) return null
    const r = await invoke<string>('set_shortcut', { shortcut: v })
    if (r === 'ok') return null
    if (r === 'conflict') return '快捷键冲突，换一个'
    if (r.startsWith('invalid')) return '格式无效，重录一次（修饰键在前、单个主键，如 Ctrl+Shift+K）'
    return r
  },

  /**
   * 贴边隐藏：鼠标碰到露在屏幕边上的那条缝，把窗口滑出来。
   * 判定全在 Rust 侧（是不是真处于收起状态），这里只管把动静传过去 —— 所以可以随便调。
   */
  async edgeReveal(): Promise<void> {
    if (!isTauri()) return
    await invoke('edge_reveal')
  },

  /** 贴边隐藏：鼠标离开窗口一会儿后收起来（窗口没贴边就没反应） */
  async edgeHide(): Promise<void> {
    if (!isTauri()) return
    await invoke('edge_hide')
  },

  // ── 窗控：走 Rust 命令，失败会返回错误文案 ────────────────────────────

  async winMinimize(): Promise<string | null> {
    return winCommand('win_minimize')
  },

  async winToggleMaximize(): Promise<string | null> {
    return winCommand('win_toggle_maximize')
  },

  /** 关闭：按设置藏到托盘或退出 */
  async winClose(): Promise<string | null> {
    return winCommand('win_close')
  },

  /** 置顶开关。成功了 Rust 那边已经把状态落进 config */
  async winSetAlwaysOnTop(value: boolean): Promise<string | null> {
    return winCommand('win_set_always_on_top', { value })
  },

  /** 标题栏按下鼠标时调用：开始拖窗口 */
  async winStartDrag(): Promise<string | null> {
    return winCommand('win_start_drag')
  },

  /**
   * 自检：确认桌面通道是通的。
   * 成功时返回 `version=...` 开头的一串环境信息，失败时返回 `连不上: ...`。
   */
  async envProbe(): Promise<string> {
    try {
      return await invoke<string>('env_probe')
    } catch (e) {
      return `连不上: ${String(e)}`
    }
  },

  /**
   * 全局快捷键到底注册上了没。
   * `ok` / `conflict`（被别的程序占了）/ `invalid`（系统不认这个组合）/ `not-registered`。
   */
  async shortcutState(): Promise<string> {
    try {
      return await invoke<string>('shortcut_state')
    } catch (e) {
      return `unknown:${String(e)}`
    }
  },

  /**
   * 导入壁纸：把选中的图**拷进数据根**，返回拷好之后的路径。
   * 失败返回 `error:...`（不抛异常）—— 界面要把原因说出来，不能"点了没反应"。
   *
   * `builtinName` 传了就是内置壁纸：拷贝出来的文件名固定成 `内置-<图名>`，
   * 界面靠它判断卡片选中态（自选的图文件名带时间戳，认不出来）。
   */
  async importWallpaper(source: string, builtinName?: string): Promise<string> {
    try {
      return await invoke<string>('import_wallpaper', {
        source,
        builtinName: builtinName ?? null,
      })
    } catch (e) {
      return `error:${String(e)}`
    }
  },

  /** 当前是不是便携模式（数据根在 exe 旁的 data 目录） */
  async isPortable(): Promise<boolean> {
    try {
      return await invoke<boolean>('is_portable')
    } catch {
      return false
    }
  },

  /** 发一条系统通知（到期提醒用）。成功返回 null，失败返回原因。 */
  async notify(title: string, body: string): Promise<string | null> {
    try {
      await invoke('notify', { title, body })
      return null
    } catch (e) {
      return String(e)
    }
  },

  /** 列出备份（新的在前），带大小与时间 */
  async backupEntries(): Promise<BackupEntry[]> {
    try {
      return await invoke<BackupEntry[]>('backup_entries')
    } catch (e) {
      logToBackend('error', `读取备份列表失败：${String(e)}`)
      return []
    }
  },

  /**
   * 从某份备份恢复待办清单。**破坏性操作**：后端会先把当前这份自动备份掉再覆盖。
   *
   * `withConfig`：这份备份若带配置快照，是否一并恢复。
   * 恢复配置时**本机路径字段会保留当前值**（清单在哪、产出根在哪这类不能跟着备份走）。
   * 成功回 `{ from, count, safety, has_config, config_restored }`，失败回 `{ error }`。
   */
  async backupRestore(path: string, withConfig: boolean): Promise<RestoreOutcome> {
    try {
      return await invoke<RestoreOutcome>('backup_restore', { path, withConfig })
    } catch (e) {
      logToBackend('error', `恢复备份失败：${String(e)}`)
      return { error: String(e) }
    }
  },

  /**
   * 开机自启的真实状态。**真相在系统那边（注册表），所以每次都问、不读本地缓存** ——
   * 用户能从任务管理器里把它关掉，缓存一份就会"显示开着、其实关了"。
   */
  async autostartInfo(): Promise<{ enabled: boolean; exe: string; debug: boolean }> {
    try {
      return await invoke<{ enabled: boolean; exe: string; debug: boolean }>('autostart_info')
    } catch (e) {
      logToBackend('error', `查询开机自启状态失败：${String(e)}`)
      return { enabled: false, exe: '', debug: false }
    }
  },

  /** 开 / 关开机自启。成功返回 null，失败返回可读原因（界面要把它说出来）。 */
  async setAutostart(enabled: boolean): Promise<string | null> {
    try {
      await invoke('autostart_set', { enabled })
      return null
    } catch (e) {
      const msg = String(e)
      logToBackend('error', `设置开机自启失败：${msg}`)
      return msg
    }
  },

  /**
   * 移除壁纸：**只删当前这一张**。
   *
   * 以前是清空整个壁纸目录 —— 用户自己放进壁纸目录的图跟着一起没了
   * （2026-09-25 修）。后端会校验这张图确实在壁纸目录内，不在就拒绝。
   * 成功返回 null，失败返回原因 —— 界面要说出来，不能"点了没反应"。
   */
  async removeWallpaper(path: string): Promise<string | null> {
    try {
      await invoke('remove_wallpaper', { path })
      return null
    } catch (e) {
      const msg = String(e)
      logToBackend('warn', `移除壁纸失败：${msg}`)
      return msg
    }
  },

  /**
   * 内置壁纸：随安装包分发的 `Wallpaper` 文件夹里的图。
   *
   * **只列不拷** —— 挑中哪张仍走 `importWallpaper` 拷进数据根。
   * 浏览器调试模式没有资源目录，返回空列表；读不出来同样返回空（已记日志），
   * 由界面显示"图该放哪儿"，绝不静默空白。
   */
  async builtinWallpapers(): Promise<BuiltinWallpaper[]> {
    if (!isTauri()) return []
    try {
      return await invoke<BuiltinWallpaper[]>('builtin_wallpapers')
    } catch (e) {
      logToBackend('warn', `读内置壁纸失败：${String(e)}`)
      return []
    }
  },

  /**
   * 用系统浏览器打开网址。后端只卡协议（http / https），不卡域名 ——
   * 别的协议（`file://` 等）会被挡掉。
   * 成功返回 null，失败返回原因 —— 界面要把它说出来。
   */
  async openUrl(url: string): Promise<string | null> {
    try {
      // key 必须叫 `url`，跟 Rust 侧 `open_url(url: String)` 对得上：
      // 之前这里传的也是 `url`、而 Rust 那边叫 `raw`，官网按钮因此一直报错打不开
      await invoke('open_url', { url })
      return null
    } catch (e) {
      const msg = String(e)
      logToBackend('warn', `打开网址失败 ${url}: ${msg}`)
      return msg
    }
  },

  /**
   * GitHub 热榜：按语言与时间范围拉热门仓库。
   * Rust 侧缓存 10 分钟；失败**抛出**（原因已可读），由界面 catch 后显示 ——
   * 错误和"还没有数据"是两回事，必须分开呈现。
   */
  async githubTrending(lang: string, since: string): Promise<GithubTrending> {
    return invoke<GithubTrending>('github_trending', { lang, since })
  },

  /** 用系统浏览器打开仓库页。成功返回 null，失败返回原因。 */
  async githubOpen(url: string): Promise<string | null> {
    try {
      await invoke('github_open', { url })
      return null
    } catch (e) {
      const msg = String(e)
      logToBackend('warn', `打开仓库失败：${msg}`)
      return msg
    }
  },

  /** 读记账数据（带指纹，保存时要原样带回去）。失败回空数据 + 日志，不静默 */
  async moneyRead(): Promise<MoneySnapshot> {
    if (!isTauri()) return { data: { version: 1, entries: [] }, fingerprint: '' }
    try {
      return await invoke<MoneySnapshot>('money_read')
    } catch (e) {
      logToBackend('error', `读记账数据失败：${String(e)}`)
      return { data: { version: 1, entries: [] }, fingerprint: '' }
    }
  },

  /**
   * 保存记账数据，带上"我这次是基于哪份改的"指纹。
   *
   * 返回 `conflict: true` 表示**盘上已经不是那一版**（多半是 AI 刚记了一笔），
   * 本次改动已被拒绝（没写进去），`latest` 里是盘上最新的 —— 界面拿它重载，
   * 让用户重做一次改动。账目被静默覆盖比弹一句"重做一次"糟得多。
   */
  async moneyWrite(data: MoneyData, baseFingerprint: string): Promise<MoneyWriteOutcome> {
    if (!isTauri())
      return { err: null, ok: true, conflict: false, fingerprint: baseFingerprint, latest: null }
    try {
      const r = await invoke<MoneyWriteResult>('money_write', {
        data,
        baseFingerprint,
      })
      return { err: null, ...r }
    } catch (e) {
      const msg = String(e)
      logToBackend('error', `保存记账失败：${msg}`)
      return { err: msg, ok: false, conflict: false, fingerprint: baseFingerprint, latest: null }
    }
  },

  /**
   * 读专注记录。失败/文件不存在都回空结构 + 日志，不静默。
   *
   * **没有指纹**：这个文件只有本应用一个写入方（见 `focus_log.rs`）。
   */
  async focusLogRead(): Promise<FocusData> {
    if (!isTauri()) return { version: 1, records: [] }
    try {
      return await invoke<FocusData>('focus_log_read')
    } catch (e) {
      logToBackend('error', `读专注记录失败：${String(e)}`)
      return { version: 1, records: [] }
    }
  },

  /** 追加一条专注记录。成功返回 null，失败返回原因（调用方要让它出现在界面上） */
  async focusLogAppend(record: FocusRecord): Promise<string | null> {
    if (!isTauri()) return null
    try {
      await invoke('focus_log_append', { record })
      return null
    } catch (e) {
      const msg = String(e)
      // 写失败必须留痕：用户只会看到"番茄钟数了但统计里没有"，分不清是坏了还是没到点
      logToBackend('warn', `写专注记录失败：${msg}`)
      return msg
    }
  },
}

/** 专注记录的一条（后端 focus_log.rs 的 FocusRecord） */
export interface FocusRecord {
  id: string
  /** 完成时刻，本地 ISO */
  at: string
  /** 本地日期 `YYYY-MM-DD` */
  date: string
  minutes: number
  todoId: string
  todoTitle: string
}

export interface FocusData {
  version: number
  records: FocusRecord[]
}

/** 记账流水的一条（后端 money.rs 的 MoneyEntry；金额单位是分） */
export interface MoneyEntry {
  id: string
  title: string
  type: 'income' | 'expense'
  amount_cents: number
  category: string
  date: string
  note: string
}

export interface MoneyData {
  version: number
  entries: MoneyEntry[]
}

/** 读回来的记账数据 + 内容指纹（后端 money.rs 的 MoneySnapshot） */
export interface MoneySnapshot {
  data: MoneyData
  fingerprint: string
}

/** 后端 money.rs 的 MoneyWriteResult */
export interface MoneyWriteResult {
  ok: boolean
  conflict: boolean
  fingerprint: string
  latest: MoneyData | null
}

/** 前端改造后的保存结果：`err` 是命令本身失败（不是冲突） */
export interface MoneyWriteOutcome extends MoneyWriteResult {
  err: string | null
}

/** GitHub 热榜的一条仓库（后端 github.rs 的 GithubRepo） */
export interface GithubRepo {
  id: number
  /** "owner/repo" */
  full_name: string
  html_url: string
  description: string
  language: string
  stargazers_count: number
  /** 日期部分 YYYY-MM-DD */
  pushed_at: string
}

export interface GithubTrending {
  items: GithubRepo[]
  /** "HH:mm"，界面显示"更新于" */
  fetched_at: string
}

/**
 * 一条备份的元信息（后端 backup_entries 的返回）。
 *
 * 备份是**文件夹**：一份 = 一个自包含的恢复单元（清单 + config.json + 壁纸）。
 * 早先的版本是平铺的单文件，那种 `legacy: true`、只有清单。
 */
export interface BackupEntry {
  path: string
  name: string
  /** 字节数（文件夹是递归算的总大小） */
  size: number
  /** 本地时间 `YYYY-MM-DD HH:mm`，后端已经格式化好了 */
  mtime: string
  /** 这份备份里有没有设置（`config.json`） */
  has_config: boolean
  /** 有没有带壁纸 */
  has_wallpaper: boolean
  /** 有没有带记账数据 */
  has_money: boolean
  /** 有没有带专注记录 */
  has_focus: boolean
  /** 老格式（平铺单文件），只有清单 */
  legacy: boolean
}

/** 恢复结果：成功给 from/count/safety，失败给 error */
export interface RestoreOutcome {
  from?: string
  count?: number
  safety?: string
  has_config?: boolean
  config_restored?: boolean
  has_wallpaper?: boolean
  wallpaper_restored?: boolean
  has_money?: boolean
  money_restored?: boolean
  has_focus?: boolean
  focus_restored?: boolean
  error?: string
}

/**
 * 一条内置壁纸（后端 `builtin_wallpapers` 的返回）。
 *
 * `path` 是**资源目录里的绝对路径**（安装目录下的 `Wallpaper` 文件夹），
 * 只用来显示缩略图和当拷贝源 —— 真正生效的壁纸永远是数据根里那份拷贝。
 */
export interface BuiltinWallpaper {
  /** 文件名（去掉扩展名），卡片上显示 */
  name: string
  /** 绝对路径 */
  path: string
}

/** 订阅"待办文件被外部改动" */
export async function onTodoFileChanged(handler: () => void): Promise<() => void> {
  if (!isTauri()) return () => {}
  // 订阅这一步也会因为权限被拒（Tauri 2 的 ACL），而且拒了是静默的 ——
  // 两头都记日志，才分得清"没订上"和"订上了但没收到"。
  try {
    const unlisten = await listen<unknown>('todo-file-changed', () => handler())
    logToBackend('info', '已订阅 todo-file-changed')
    return unlisten
  } catch (e) {
    logToBackend('error', `订阅 todo-file-changed 失败：${String(e)}`)
    return () => {}
  }
}
