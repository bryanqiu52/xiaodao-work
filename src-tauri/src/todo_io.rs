//! 待办文件的读写。
//!
//! **Rust 不解析待办的业务语义** —— 只做字节级读写，业务规则全在前端 `src/core/`。
//! 这里唯一重要的是"别把别人的改动盖掉"：
//!
//!   - 写前**重读**文件，比对前端上次读到的指纹；
//!   - 不一致 → 拒绝写入并回传盘上最新内容，让前端重新加载（而不是硬覆盖）；
//!   - 一致 → tmp + rename 原子写。
//!
//! 锁只防本应用自身的并发（single-instance 已保证单实例）。跨端（AI 直接改文件）
//! 管不住也管不了，靠上面那套重读比对兜底。

use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

use crate::fs;

static IO_LOCK: Mutex<()> = Mutex::new(());

/// 最近一次**本应用**写入后的内容指纹：文件监听用它过滤掉自己造成的回环
static LAST_WRITE: Mutex<Option<String>> = Mutex::new(None);

/// 文件不存在时的指纹（空串）——与"空文件"区分开
pub const MISSING_FINGERPRINT: &str = "";

#[derive(Debug, Clone, Serialize)]
pub struct TodoSnapshot {
    /// 文件原文（前端自己 JSON.parse）
    pub text: String,
    /// 内容指纹，写入时要原样带回来
    pub fingerprint: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WriteResult {
    pub ok: bool,
    /// 冲突：盘上内容和前端看到的不是同一份
    pub conflict: bool,
    /// 写入成功后的新指纹（冲突时是盘上当前的指纹）
    pub fingerprint: String,
    /// 冲突时回传盘上最新原文，前端拿它直接重载
    pub latest: Option<String>,
}

/// 读待办文件。文件不存在不报错 —— 返回 exists=false，前端按空清单处理。
pub fn read_todo(path: &Path) -> Result<TodoSnapshot, String> {
    if !path.is_file() {
        return Ok(TodoSnapshot {
            text: String::new(),
            fingerprint: MISSING_FINGERPRINT.to_string(),
            exists: false,
        });
    }
    let text = fs::read_text(path)?;
    let fingerprint = fs::fingerprint(text.as_bytes());
    Ok(TodoSnapshot {
        text,
        fingerprint,
        exists: true,
    })
}

/// 写待办文件。
///
/// `base_fingerprint` 是前端这次改动**所基于**的那份内容指纹。
/// 盘上不是这一份就拒绝写入 —— 否则会把 AI 刚写进去的东西整份盖掉。
pub fn write_todo(path: &Path, text: &str, base_fingerprint: &str) -> Result<WriteResult, String> {
    let _guard = IO_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let current = read_todo(path)?;
    if current.fingerprint != base_fingerprint {
        log::warn!(
            "[待办] 写入被拒绝：盘上内容已变（期望 {} 实际 {}）",
            if base_fingerprint.is_empty() { "空" } else { base_fingerprint },
            if current.fingerprint.is_empty() { "空" } else { &current.fingerprint }
        );
        return Ok(WriteResult {
            ok: false,
            conflict: true,
            fingerprint: current.fingerprint,
            latest: Some(current.text),
        });
    }

    fs::atomic_write(path, text.as_bytes())?;
    let new_fp = fs::fingerprint(text.as_bytes());
    if let Ok(mut slot) = LAST_WRITE.lock() {
        *slot = Some(new_fp.clone());
    }
    Ok(WriteResult {
        ok: true,
        conflict: false,
        fingerprint: new_fp,
        latest: None,
    })
}

/// 本应用最近一次写入的指纹（供文件监听过滤自己的回声）
pub fn last_write_fingerprint() -> Option<String> {
    LAST_WRITE.lock().ok().and_then(|g| g.clone())
}

/// 登记"这份内容是本应用刚写的"。
///
/// 恢复备份时用：恢复的语义是「无视盘上内容、按备份整份覆盖」，
/// 所以它**刻意不走** `write_todo` 那套指纹比对（那是给"基于当前内容做的改动"用的）。
/// 但写完必须登记指纹 —— 否则文件监听会把这次覆盖当成"外部改动"，
/// 再通知前端重载一轮，白跑一趟。
pub fn note_write(text: &str) {
    if let Ok(mut slot) = LAST_WRITE.lock() {
        *slot = Some(fs::fingerprint(text.as_bytes()));
    }
}
