//! 监听数据文件被外部改动（AI 直接改文件 / 别的进程写回）→ 通知前端重载。
//!
//! 待办和记账都监听：这两个都可能被本应用之外的一方改动 —— 待办是 AI / 插件，
//! 记账是 AI（走 skill 的脚本）。**只加指纹不监听的话，AI 记了一笔界面上看不到**，
//! 等于白给；只监听不比对指纹又会陷入 写 → 监听 → 重载 → 又写 的回环。
//!
//! **专注记录（`专注记录.json`）刻意不在这里监听**：它的写入方只有本应用一个，
//! 上面那两个"监听"的理由（AI 会改）对它一条都不成立。加一条只是白养一个线程，
//! 永远收不到"外部改动"。
//!
//! 三个坑都在这里堵：
//!   1. **回环**：本应用自己写完会触发一次文件事件，若不比对指纹就死循环。
//!   2. **编辑器原子替换**：有些编辑器保存是先写 tmp 再 rename，监听单文件会失效。
//!      所以**监听所在目录**，再按文件名过滤。
//!   3. **自举**：记账文件可能压根不存在（还没记过账）。监听的是所在目录，
//!      文件被创建时一样收得到事件，不会漏。

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::money;
use crate::todo_io;

/// 去抖窗口：连续事件静默这么久才算"改完了"
const DEBOUNCE: Duration = Duration::from_millis(300);

/// 监听哪一类文件。
///
/// 两者的监听机制（目录 + 文件名过滤 + 去抖）完全一样，
/// 差的是"怎么读"和"怎么判断是不是自己写的"，所以共用一套、只分派这两步
#[derive(Clone, Copy)]
pub enum WatchKind {
    Todo,
    Money,
}

impl WatchKind {
    fn label(self) -> &'static str {
        match self {
            WatchKind::Todo => "待办",
            WatchKind::Money => "记账",
        }
    }
}

pub fn start(app: &AppHandle, file: PathBuf) {
    spawn(app, file, WatchKind::Todo)
}

/// 监听记账文件（路径由 `paths::money_file()` 定，在数据根里）
pub fn start_money(app: &AppHandle) {
    spawn(app, crate::paths::money_file(), WatchKind::Money)
}

fn spawn(app: &AppHandle, file: PathBuf, kind: WatchKind) {
    let handle = app.clone();
    thread::spawn(move || {
        if let Err(e) = run(&handle, &file, kind) {
            log::warn!("[监听] 已停止: {}", e);
        }
    });
}

fn run(app: &AppHandle, file: &Path, kind: WatchKind) -> Result<(), String> {
    let dir = file
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| format!("{}文件路径没有父目录，无法监听", kind.label()))?;
    let file_name = file
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if file_name.is_empty() {
        return Err(format!("{}文件路径没有文件名", kind.label()));
    }

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
        .map_err(|e| format!("创建文件监听失败: {}", e))?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("监听目录失败 {}: {}", dir.display(), e))?;

    log::info!("[监听] 已监听{}文件: {}", kind.label(), file.display());

    // 事件进来只打标记，等静默 300ms 再处理一次（一次保存往往触发好几个事件）
    let mut dirty = false;
    loop {
        match rx.recv_timeout(DEBOUNCE) {
            Ok(Ok(event)) => {
                if relevant(&event, &file_name) {
                    dirty = true;
                }
            }
            Ok(Err(e)) => log::warn!("[监听] 事件错误: {}", e),
            Err(RecvTimeoutError::Timeout) => {
                if dirty {
                    dirty = false;
                    handle_change(app, file, kind);
                }
            }
            Err(RecvTimeoutError::Disconnected) => return Err("监听通道已断开".to_string()),
        }
    }
}

fn relevant(event: &Event, file_name: &str) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    event
        .paths
        .iter()
        .any(|p| p.file_name().map(|n| n.to_string_lossy() == file_name).unwrap_or(false))
}

/// 静默期结束：读一次文件，确认不是自己的回声，再通知前端
fn handle_change(app: &AppHandle, file: &Path, kind: WatchKind) {
    let label = kind.label();
    match kind {
        WatchKind::Todo => {
            let snapshot = match todo_io::read_todo(file) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("[监听] 读取{}文件失败: {}", label, e);
                    return;
                }
            };
            // 自己刚写的：指纹一致，忽略
            if let Some(last) = todo_io::last_write_fingerprint() {
                if last == snapshot.fingerprint {
                    return;
                }
            }
            log::info!("[监听] {}文件被外部改动，通知前端重载", label);
            // 派发结果要记：之前这里 `let _ =` 吞掉错误，出了事只能猜"是没发出去还是没收着"
            match app.emit("todo-file-changed", snapshot.fingerprint) {
                Ok(_) => log::info!("[监听] 事件已派发"),
                Err(e) => log::warn!("[监听] 派发事件失败: {}", e),
            }
        }
        WatchKind::Money => {
            let snapshot = money::read_snapshot();
            if let Some(last) = money::last_write_fingerprint() {
                if last == snapshot.fingerprint {
                    return;
                }
            }
            log::info!("[监听] {}文件被外部改动，通知前端重载", label);
            match app.emit("money-file-changed", snapshot.fingerprint) {
                Ok(_) => log::info!("[监听] 事件已派发"),
                Err(e) => log::warn!("[监听] 派发事件失败: {}", e),
            }
        }
    }
}
