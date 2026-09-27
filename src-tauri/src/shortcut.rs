//! 全局快捷键：默认 `Ctrl+Shift+Space`，一键唤起 / 隐藏主窗。
//!
//! 注册失败要**分清楚原因**再告诉用户：
//!   - 冲突（`conflict`）：这个组合被别的程序占了，换一个；
//!   - 格式无效（`invalid`）：拼得不对，重录一次。
//! 而且失败时要把旧的装回去 —— 不然改快捷键改崩了，就再也没有唤起方式了。

use std::str::FromStr;
use std::sync::Mutex;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config;
use crate::tray::toggle_window;

/// 最近一次注册的结果：`ok` / `conflict` / `invalid` / `not-registered`。
///
/// **为什么要记下来**：注册失败原来只在日志里 warn 一句，界面上完全无感知 ——
/// 用户只会发现"快捷键没反应"，却不知道是被人占了。这个状态给设置页和启动提示用。
static STATE: Mutex<Option<String>> = Mutex::new(None);

fn note(result: &Result<(), String>) {
    let text = match result {
        Ok(()) => "ok".to_string(),
        Err(reason) => reason.clone(),
    };
    if let Ok(mut g) = STATE.lock() {
        *g = Some(text);
    }
}

/// 给前端查：快捷键到底注册上没
pub fn state() -> String {
    STATE
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "not-registered".to_string())
}

/// 注册（启动时用）：失败只 warn，绝不阻塞启动
pub fn register(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let sc = match Shortcut::from_str(shortcut) {
        Ok(sc) => sc,
        Err(_) => {
            let r = Err("invalid".to_string());
            note(&r);
            return r;
        }
    };
    let r = app
        .global_shortcut()
        .register(sc)
        .map_err(|e| classify(&e.to_string()));
    note(&r);
    r
}

/// 改快捷键：先卸旧的再装新的，装不上就把旧的装回去
pub fn change(app: &AppHandle, new_shortcut: &str) -> Result<(), String> {
    let old = config::load().global_shortcut;
    let new_sc = match Shortcut::from_str(new_shortcut) {
        Ok(sc) => sc,
        Err(_) => {
            let r = Err("invalid".to_string());
            note(&r);
            return r;
        }
    };

    if let Ok(old_sc) = Shortcut::from_str(&old) {
        let _ = app.global_shortcut().unregister(old_sc);
    }

    let r = match app.global_shortcut().register(new_sc) {
        Ok(()) => Ok(()),
        Err(e) => {
            let reason = classify(&e.to_string());
            // 装回旧的，保证至少还有一种唤起方式
            if let Ok(old_sc) = Shortcut::from_str(&old) {
                let _ = app.global_shortcut().register(old_sc);
            }
            Err(reason)
        }
    };
    note(&r);
    r
}

/// 插件的按键回调：只在**按下**时触发（抬起也响应的话，长按会来回闪）
pub fn on_shortcut(app: &AppHandle, _shortcut: &Shortcut, state: ShortcutState) {
    if state == ShortcutState::Pressed {
        toggle_window(app);
    }
}

/// 把注册失败的原因归成两类：冲突 / 格式无效
fn classify(msg: &str) -> String {
    let lower = msg.to_lowercase();
    if lower.contains("already") || lower.contains("conflict") || lower.contains("registered") {
        "conflict".to_string()
    } else {
        format!("invalid:{}", msg)
    }
}
