//! 记账数据：`数据根\记账.json`。
//!
//! **有两个写入方**：本应用 + AI（通过 `xiaodao-work` skill 的脚本）。
//!
//! 所以**照 `todo_io` 那套指纹比对来做**：写入时必须带上"我这次改动是基于哪份内容"
//! 的指纹，写前重读一次，盘上不是那一版就直接拒绝，而不是整份盖掉。
//! 少了这一步就会变成：AI 记一笔 → 用户在界面上再记一笔 → 界面拿它内存里的旧快照
//! 整份写回 → **AI 那笔没了**（反过来也一样）。账目少一笔比待办少一条更难发现，
//! 所以这里用和待办同一套最严的规矩。
//!
//! 金额存**分**（整数）：元做浮点统计会出 `0.30000000000000004` 这种尾巴，
//! 账目对不上分毫都是事。

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 最近一次**本应用**写入后的内容指纹：文件监听用它过滤掉自己造成的回环
static LAST_WRITE: Mutex<Option<String>> = Mutex::new(None);

/// 文件不存在时的指纹（空串）—— 与"空文件"区分开。与 `todo_io` 同口径
pub const MISSING_FINGERPRINT: &str = "";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyEntry {
    pub id: String,
    pub title: String,
    /// income / expense
    #[serde(rename = "type")]
    pub kind: String,
    /// 金额，单位**分**（整数，恒非负；类型由 kind 决定正负语义）
    pub amount_cents: i64,
    pub category: String,
    /// YYYY-MM-DD
    pub date: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoneyData {
    pub version: u32,
    pub entries: Vec<MoneyEntry>,
}

/// 读回来的记账数据 + 它的内容指纹。写入时指纹要原样带回来（见 `write`）
#[derive(Debug, Clone, Serialize)]
pub struct MoneySnapshot {
    pub data: MoneyData,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoneyWriteResult {
    pub ok: bool,
    /// 冲突：盘上内容和这次改动所基于的不是同一份
    pub conflict: bool,
    /// 写入成功后的新指纹（冲突时是盘上当前的指纹）
    pub fingerprint: String,
    /// 冲突时回传盘上最新数据，前端拿它直接重载，不用再读一次
    pub latest: Option<MoneyData>,
}

/// 读记账数据。文件不存在 = 还没记过账（不是错误）；
/// 解析失败回空数据但**指纹照记盘上那份** —— 坏文件也是"盘上现状"，
/// 当成空会让人以为可以放心整份覆盖，那正是要防的事
pub fn read_snapshot() -> MoneySnapshot {
    let path = crate::paths::money_file();
    match crate::fs::read_text(&path) {
        Ok(text) => {
            let fingerprint = crate::fs::fingerprint(text.as_bytes());
            match serde_json::from_str::<MoneyData>(&text) {
                Ok(data) => MoneySnapshot { data, fingerprint },
                Err(e) => {
                    log::warn!("[记账] 解析失败（{}），已回退空数据: {}", path.display(), e);
                    MoneySnapshot {
                        data: MoneyData::default(),
                        fingerprint,
                    }
                }
            }
        }
        Err(_) => MoneySnapshot {
            data: MoneyData::default(),
            fingerprint: MISSING_FINGERPRINT.to_string(),
        },
    }
}

/// 写记账数据。
///
/// `base_fingerprint` 是这次改动**所基于**的那份内容指纹。
/// 盘上不是这一份就**拒绝写入**并把盘上最新的回传回去 —— 绝不整份盖掉，
/// 否则会把 AI（或界面上另一次操作）刚记进去的那一笔冲掉。
pub fn write(data: &MoneyData, base_fingerprint: &str) -> Result<MoneyWriteResult, String> {
    let path = crate::paths::money_file();

    // 写前重读：这里隔了一次前端往返，期间完全可能有人（AI / 另一次保存）改过
    let current = read_snapshot();
    if current.fingerprint != base_fingerprint {
        log::warn!(
            "[记账] 写入被拒绝：盘上内容已变（期望 {} 实际 {}），已把最新的回传",
            if base_fingerprint.is_empty() { "空" } else { base_fingerprint },
            if current.fingerprint.is_empty() { "空" } else { &current.fingerprint }
        );
        return Ok(MoneyWriteResult {
            ok: false,
            conflict: true,
            fingerprint: current.fingerprint,
            latest: Some(current.data),
        });
    }

    let text =
        serde_json::to_string_pretty(data).map_err(|e| format!("序列化记账数据失败: {}", e))?;
    crate::fs::atomic_write(&path, text.as_bytes())?;

    let new_fp = crate::fs::fingerprint(text.as_bytes());
    if let Ok(mut slot) = LAST_WRITE.lock() {
        *slot = Some(new_fp.clone());
    }
    Ok(MoneyWriteResult {
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
/// **恢复备份时用**：恢复的语义是「无视盘上内容、按备份整份覆盖」，
/// 所以它**刻意不走** `write` 那套指纹比对（那是给"基于当前内容做的改动"用的）。
/// 但写完必须登记 —— 否则文件监听会把这次覆盖当成"外部改动"，又通知前端重载一轮。
pub fn note_write(text: &str) {
    if let Ok(mut slot) = LAST_WRITE.lock() {
        *slot = Some(crate::fs::fingerprint(text.as_bytes()));
    }
}
