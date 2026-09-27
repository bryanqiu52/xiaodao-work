//! 到期提醒的"节拍器"：每半小时往前端喊一声 `reminder-tick`。
//!
//! **为什么这条线程放在 Rust，而不是前端一个 setInterval**：
//! WebView 在窗口隐藏 / 最小化时会把定时器节流（Chrome 那套省电策略），
//! 而"关掉窗口藏进托盘"正是本应用最常见的用法 —— 前端定时器一旦被节流，
//! 提醒就会迟迟不来甚至不来。Rust 这边发事件不受那套策略影响。
//!
//! **这里不做任何判定**：该提醒谁、通知上写什么，全是业务，留在前端 `core/remind.ts`。
//! 这条线程只负责"到点了，去看一眼"。

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

/// 检查间隔：半小时。
///
/// 为什么不更密：提醒是"天"粒度的（提前 0–3 天），半小时查一次已经很够了。
/// 更密只会白耗电，还容易让"同一天提醒多次"这种 bug 更容易暴露。
const INTERVAL: Duration = Duration::from_secs(30 * 60);

/// 启动后第一次检查的延迟：等一等，别跟应用启动抢那一下资源
const FIRST_DELAY: Duration = Duration::from_secs(15);

pub fn start(app: &AppHandle) {
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(FIRST_DELAY);
        loop {
            // 只喊一声，具体判不判、提不提醒由前端决定（它还要看开关和"今天提醒过没"）
            if let Err(e) = handle.emit("reminder-tick", ()) {
                log::warn!("[提醒] 派发 reminder-tick 失败: {}", e);
            }
            thread::sleep(INTERVAL);
        }
    });
}

/// 番茄钟的到点兜底拍子：每 15 秒喊一声 `focus-tick`。
///
/// 番茄钟的计时在前端（结束时间戳），前台时它自己的 250ms interval 就够；
/// 但窗口藏进托盘 / 最小化后 WebView 的定时器会被节流到分钟级 ——
/// 番茄钟到点的通知就会迟到。事件派发不受节流影响，所以照 `reminder-tick`
/// 同样的路子再开一条更密的拍子，兜住隐藏状态下的到点检测。
/// 只在番茄钟跑着时才有人消费它，空转的开销可以忽略。
pub fn start_focus_tick(app: &AppHandle) {
    let handle = app.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(15));
        if let Err(e) = handle.emit("focus-tick", ()) {
            log::warn!("[提醒] 派发 focus-tick 失败: {}", e);
        }
    });
}
