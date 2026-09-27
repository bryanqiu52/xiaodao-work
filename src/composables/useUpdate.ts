// 检查更新 / 下载安装。
//
// 底层是 Tauri 的官方 updater 插件：它去 `tauri.conf.json` 里配的那个地址
// （本仓库 Releases 上的 `latest.json`）读最新版，拿到之后**校验签名** ——
// 签名对不上就不装。这一步不是可选项：没有它，任何人换掉 release 上的安装包
// 都能推给用户。公钥写在配置里（公开的），私钥只在 CI 的 Secret 里。
//
// 三条产品上的取舍（都写在下面各自的位置）：
//   1. **一天最多自动查一次** —— 每次启动都打 GitHub 不礼貌，匿名接口还有速率限制；
//   2. **同一个版本只弹一次** —— 用户说了"先不更新"就别拿同一件事反复烦他，
//      但出现更新的版本时照样弹（那说明又有新东西了）；
//   3. **失败必须留痕** —— 查不到更新和"查询本身失败了"是两件事，
//      后者要写进日志，否则用户会以为"我这版就是最新的"。

import { reactive } from 'vue'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { isTauri, logToBackend } from '../api/tauri'
import { configStore, saveConfig } from '../stores/config'
import { todayStr } from './useReminder'

export interface UpdateInfo {
  version: string
  /** 这个版本的说明（就是 Release 正文，来自 latest.json 的 notes） */
  body: string
  date: string
}

export const updateState = reactive({
  checking: false,
  /** 有新版本才有值；null = 没有（或者还没查过） */
  info: null as UpdateInfo | null,
  /** 用户点过「这次先不更新」（同一版本内不再自动弹） */
  dismissed: false,
  downloading: false,
  /** 下载进度 0–100；负数 = 总大小未知，界面上走"不确定"那种显示 */
  progress: -1,
  /** 出错了才有值 —— 界面上要说出来，别让它静默 */
  error: '',
  /** 手动检查之后的一句话反馈（没更新时也得有回应，不然像坏了） */
  manualNote: '',
})

/** 插件返回的那个 update 句柄，下载要用它 */
let pending: Update | null = null

/** 手动查是不是该跳过节流 */
function shouldAutoCheck(): boolean {
  return configStore.cfg.update_checked_at !== todayStr()
}

/**
 * 查一次更新。
 *
 * - `manual: true` —— 用户点的：**忽略"一天一次"的节流**，且无论有没有新版都给一句反馈
 * - 自动查时失败**只落日志**（不弹任何东西）：网络抖一下就弹个错误窗口太打扰
 */
export async function checkUpdate(manual = false): Promise<void> {
  if (updateState.checking) return
  if (!isTauri()) {
    // 浏览器调试模式没有 updater（它要读签名、要跑安装包），说清楚而不是假装查过了
    if (manual) updateState.manualNote = '浏览器调试模式里没有更新功能，桌面版才有'
    return
  }
  if (!manual && !shouldAutoCheck()) return

  updateState.checking = true
  updateState.error = ''
  updateState.manualNote = ''
  try {
    const found = await check()
    // 查成功的这一次才记日期：失败不记，下次启动还能再试
    await saveConfig({ update_checked_at: todayStr() })

    if (found === null) {
      pending = null
      updateState.info = null
      if (manual) updateState.manualNote = '已经是最新版了'
      logToBackend('info', '检查更新：已是最新版')
      return
    }

    pending = found
    updateState.info = {
      version: found.version,
      body: found.body ?? '',
      date: found.date ?? '',
    }
    // 用户之前说过"这一版先不更新" → 不自动弹；手动查的仍然给看
    updateState.dismissed = !manual && configStore.cfg.update_skipped_version === found.version
    logToBackend(
      'info',
      `检查更新：发现新版本 ${found.version}${updateState.dismissed ? '（用户已跳过这一版，不弹）' : ''}`,
    )
  } catch (e) {
    const msg = String(e)
    updateState.error = msg
    // 静默失败最费命：用户会以为"我这版就是最新的"
    logToBackend('warn', `检查更新失败：${msg}`)
    if (manual) updateState.manualNote = `检查失败：${msg}`
  } finally {
    updateState.checking = false
  }
}

/** 用户点了「这次先不更新」：记住这一版，别再自动弹同一个 */
export async function dismissUpdate(): Promise<void> {
  const v = updateState.info?.version ?? ''
  updateState.dismissed = true
  if (v.length > 0) await saveConfig({ update_skipped_version: v })
}

/**
 * 下载并安装，装完自动重启。
 *
 * 进度用"已收字节 / 总字节"算；**总大小拿不到时给 -1**（服务端没报 content-length），
 * 界面据此显示"不确定进度"—— 硬凑一个百分比出来是在骗人。
 */
export async function installUpdate(): Promise<void> {
  if (pending === null || updateState.downloading) return
  updateState.downloading = true
  updateState.progress = -1
  updateState.error = ''
  let total = 0
  let received = 0
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        total = event.data.contentLength ?? 0
      } else if (event.event === 'Progress') {
        received += event.data.chunkLength
        updateState.progress = total > 0 ? Math.round((received / total) * 100) : -1
      } else {
        // Finished：下载完了，插件去跑安装包
        updateState.progress = 100
      }
    })
    logToBackend('info', `更新 ${updateState.info?.version ?? ''} 安装完成，正在重启`)
    // 装完要自己把应用拉起来 —— updater 只管装，不管重启
    await relaunch()
  } catch (e) {
    const msg = String(e)
    updateState.error = msg
    // 装失败必须说：用户以为在更新，其实什么都没发生
    logToBackend('warn', `安装更新失败：${msg}`)
  } finally {
    updateState.downloading = false
  }
}
