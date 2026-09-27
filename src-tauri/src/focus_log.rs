//! 专注记录：`数据根\专注记录.json`。
//!
//! 记的是"哪个番茄花在哪条待办上"：完成时刻、本地日期、分钟数、待办编号与标题。
//! 番茄钟界面显示"这条累计几个番茄"、回顾页统计"这段时间专注了多久"，都读它。
//!
//! **为什么单独一个文件、不写进 `待办.json`**：那份清单是三方共写的
//! （本应用 + DSH 插件 + AI skill），加字段等于改契约，另外两方都得跟着动。
//! 而专注时长是本应用自己的账，放自己家文件里更合适。
//! 代价是小刀在 AI 侧看不到专注时长 —— 这个取舍是明确选定的。
//!
//! **两条与 `money.rs` 刻意不同的地方**（改之前先读）：
//!
//! 1. **不做指纹比对**。那套是给"多方共写"用的（待办三方、记账两方）。
//!    这里只有本应用一个写入方，比对没有对象；但**仍然要 `Mutex` + 读改写 + 原子写** ——
//!    同进程里两处同时追加（专注到点 + 用户手动记）依然会互相覆盖。
//! 2. **不进 `watcher` 监听**。监听的两个文件都有"AI 会改"这个前提，
//!    专注记录没有。加一条监听只是白养一个线程，收不到任何外部改动。

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 同进程内的写串行化。只用一把裸锁：这里没有跨进程并发
/// （多开由 single-instance 插件顶掉第二个进程）。
static LOCK: Mutex<()> = Mutex::new(());

/// 取锁。**中毒（前一次持锁时 panic）不当作致命错误** ——
/// 记录的是番茄钟这种"丢了也就少一段统计"的数据，
/// 因为一把锁中毒就让功能永久瘫掉，比丢一条记录糟得多。
fn lock() -> std::sync::MutexGuard<'static, ()> {
    match LOCK.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
}

const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusRecord {
    pub id: String,
    /// 完成时刻（本地 ISO）。前端算好传下来 —— 时区判断属于业务语义，Rust 不碰
    #[serde(default)]
    pub at: String,
    /// 本地日期 `YYYY-MM-DD`。统计全靠它，所以单独存一格，不从 `at` 现算
    #[serde(default)]
    pub date: String,
    /// 这一段有几分钟
    #[serde(default)]
    pub minutes: u32,
    /// 挂在哪条待办上；没挂就是空串
    #[serde(rename = "todoId", default)]
    pub todo_id: String,
    /// **冗余存一份标题**：待办改名或软删除之后，历史统计还能显示当时干的是什么
    #[serde(rename = "todoTitle", default)]
    pub todo_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusData {
    pub version: u32,
    pub records: Vec<FocusRecord>,
}

impl Default for FocusData {
    fn default() -> Self {
        Self {
            version: VERSION,
            records: Vec::new(),
        }
    }
}

/// 读专注记录。
///
/// 文件不存在 = 还没专注过（**不是错误**，正常返回空）；
/// 解析失败也回空结构，但**会记一条 warn** —— 坏文件被当成空之后，
/// 下一次写入就会把盘上那份整个盖掉，留个日志至少事后能看出是怎么没的。
pub fn read() -> FocusData {
    let path = crate::paths::focus_file();
    let text = match crate::fs::read_text(&path) {
        Ok(t) => t,
        Err(_) => return FocusData::default(),
    };
    match serde_json::from_str::<FocusData>(&text) {
        Ok(data) => data,
        Err(e) => {
            log::warn!(
                "[专注记录] 解析失败（{}），本次按空处理: {}",
                path.display(),
                e
            );
            FocusData::default()
        }
    }
}

/// 追加一条专注记录。
///
/// 「读全文 + 追加 + 全量写」：单条约 150 字节，一年每天几个番茄也就几十 KB，
/// 全量写在可接受范围内。不做增量插入 —— JSON 数组没有安全的追加语义，
/// 为省这点 IO 去啃"就地插一段"，出错的代价远大于收益（`money.rs` 同样取舍）。
pub fn append(record: FocusRecord) -> Result<(), String> {
    let _guard = lock();
    let mut data = read();
    data.records.push(record);
    data.version = VERSION;
    write(&data)
}

/// 清空（「恢复出厂设置」用）。
///
/// **写空结构而不是删文件**：与 `data_reset` 清记账同一口径 ——
/// 文件还在、内容为空，比"文件没了"更容易分辨"清过了"和"从来没用过"。
pub fn clear() -> Result<(), String> {
    let _guard = lock();
    write(&FocusData::default())
}

fn write(data: &FocusData) -> Result<(), String> {
    let path = crate::paths::focus_file();
    let text = serde_json::to_string_pretty(data)
        .map_err(|e| format!("序列化专注记录失败: {}", e))?;
    crate::fs::atomic_write(&path, text.as_bytes())
}
