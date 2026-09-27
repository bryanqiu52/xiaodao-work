//! 开机自启：把本程序注册进系统启动项（Windows 是注册表 Run 键），开机后托盘待命。
//!
//! **状态一律问插件，绝不往 config 里存第二份。**
//! 真相在系统那边（注册表），用户完全可能从任务管理器里把它关掉。
//! config 里再存一份就会漂移 —— 界面显示"已开启"、实际早被关了。
//! 这个项目里"显示的和真实的不一致"已经咬过人（快捷键那次：注册失败只在日志里说）。

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

#[derive(Serialize)]
pub struct AutostartInfo {
    /// 系统里是不是真的开着（直接问插件，不读缓存）
    pub enabled: bool,
    /// 当前注册进去的可执行文件路径
    pub exe: String,
    /// 是不是开发版。开发版注册的是 `target\debug\` 下的 exe ——
    /// 那是个临时产物，删了就失效，得在界面上直说
    pub debug: bool,
}

#[tauri::command]
pub fn autostart_info(app: AppHandle) -> AutostartInfo {
    let enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    AutostartInfo {
        enabled,
        exe,
        debug: cfg!(debug_assertions),
    }
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => {
            log::info!("[自启] {}", if enabled { "已开启" } else { "已关闭" });
            Ok(())
        }
        Err(e) => {
            // 失败原因要回给界面：用户点了开关却没生效，必须知道为什么
            let msg = e.to_string();
            log::warn!("[自启] 设置失败: {}", msg);
            Err(msg)
        }
    }
}
