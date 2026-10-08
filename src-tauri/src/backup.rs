//! 备份：把「清单 + 配置 + 壁纸」打包成一个文件夹，存到 `数据根\待办备份\`，保留若干份。
//!
//! **为什么是文件夹而不是平铺的文件**：一份备份该是一个**自包含的恢复单元** ——
//! 挑一份出来，清单、界面设置、壁纸全在里面，整份挪到哪台机器都是完整的。
//! 平铺时只能靠文件名配对（`待办-x.json` 配 `待办-x.config.json`），
//! 想再加一样东西就排不下了；而且两份不同的东西都叫 `待办-x` 本身就含混。
//!
//! 目录长这样：
//!
//! ```text
//! 待办备份/
//!   2026-09-24/              每日备份（当天第一次启动时）
//!     待办.json
//!     config.json
//!     记账.json
//!     专注记录.json
//!     壁纸/
//!       壁纸-20260924-011152.jpg
//!   2026-09-24-093012/       当天的「立即备份」（当天已有则在后面加时间）
//!   待办-2026-09-23.json     老格式：平铺的单文件备份
//! ```
//!
//! **带哪几样是有讲究的**：清单三方共写，是主角；设置、壁纸、记账、专注记录
//! 都是"本应用这一份，丢了就没有第二份"，所以一并收进来。
//! 后两样的共同点：写入方都只有本应用或本应用 + AI，从来没有别人替它们存过档。
//!
//! **老格式照样认、照样能恢复**，只是不再产生新的 —— 用户手上的旧备份
//! 不能因为改了结构就作废。
//!
//! **调用时机有讲究**：必须在**加载之前**跑。加载时会就地修正完成态并写回文件，
//! 备份必须落在修正之前，否则回滚凭据就不是"修改前"的样子（这个坑插件里踩过并写了注释）。

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;
use crate::todo_io;

/// 备份文件夹里的固定文件名
const F_TODO: &str = "待办.json";
const F_CONFIG: &str = "config.json";
const F_WALLPAPER: &str = "壁纸";
const F_MONEY: &str = "记账.json";
const F_FOCUS: &str = "专注记录.json";

/// 恢复配置时**必须保留本机值**的字段。
///
/// 这几个都指向"这台机器上的某个具体位置"。换台机器（或换了目录）之后，
/// 备份里那份的值多半不存在了 —— 照它恢复，轻则"打开产出"全失效。
///
/// 所以：**行为设置跟着备份走，路径设置跟着本机走**。
///
/// `wallpaper_path` 也在其中，但它有个额外处理：备份里**带了壁纸**时，
/// 恢复过程会把图拷进当前数据根、并把这个字段改写成新位置（见 `restore_wallpaper`）。
///
/// `notified_data_root` 也属于本机状态：它记的是"上次告诉 AI 的路径是哪个"，
/// 跟着备份走的话，恢复旧配置后会拿一个过时的值去跟现在比，
/// 生成出来的"从…变为…"就是错的。
///
/// （`todo_file` 原来也在里面，2026-09-30 整合后它不是配置项了 ——
///   待办文件固定跟着数据根走，不会再被备份里的旧值指偏。）
const MACHINE_SPECIFIC_KEYS: [&str; 6] = [
    "library_root",
    "preview_exe",
    "open_roots",
    "window",
    "wallpaper_path",
    "notified_data_root",
];

// ── 名字识别 ──────────────────────────────────────────────────────────────

/// 新的备份文件夹名：`2026-09-24` 或 `2026-09-24-093012`
fn is_day_dir_name(name: &str) -> bool {
    let b = name.as_bytes();
    let date_ok = b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..10].iter().all(|c| c.is_ascii_digit() || *c == b'-');
    if !date_ok {
        return false;
    }
    if b.len() == 10 {
        return true;
    }
    // 带后缀时必须是 -HHMMSS
    b.len() == 17 && b[10] == b'-' && b[11..].iter().all(|c| c.is_ascii_digit())
}

/// 老格式的备份文件名：`待办-YYYY-MM-DD.json` / `待办-YYYY-MM-DD-HHMMSS.json`
fn is_legacy_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("待办-") else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".json") else {
        return false;
    };
    is_day_dir_name(stem)
}

/// 这个目录项算不算一份备份（新的文件夹 or 老的单文件都算）
fn is_backup_entry(p: &Path) -> bool {
    let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if p.is_dir() {
        return is_day_dir_name(name);
    }
    is_legacy_name(name)
}

// ── 写备份 ────────────────────────────────────────────────────────────────

/// 把当前壁纸拷进备份文件夹（没设壁纸、或者图不在了就跳过）。
///
/// **只拷当前这一张**，不是把 `壁纸/` 目录整个搬走 —— 那里面可能堆着用户换过的旧图，
/// 备份它纯属浪费。失败只记日志：壁纸是锦上添花，不该拖累清单备份。
fn copy_wallpaper_in(target: &Path) {
    let path = crate::config::load().wallpaper_path;
    if path.trim().is_empty() {
        return;
    }
    let src = PathBuf::from(&path);
    if !src.is_file() {
        log::warn!("[备份] 壁纸文件不在了，跳过: {}", path);
        return;
    }
    let dir = target.join(F_WALLPAPER);
    if let Err(e) = fs::create_dir_all(&dir) {
        log::warn!("[备份] 建壁纸目录失败（不影响清单备份）: {}", e);
        return;
    }
    let name = src
        .file_name()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| std::ffi::OsString::from("wallpaper"));
    if let Err(e) = fs::copy(&src, dir.join(name)) {
        log::warn!("[备份] 壁纸没拷进去（不影响清单备份）: {}", e);
    }
}

/// 建一份新备份（文件夹形式），返回它的路径。
///
/// 配置和壁纸都是**尽力而为**：失败只记日志，不把整次备份判失败 ——
/// 清单才是主角，为了附件让主流程失败，反而会害用户丢掉真正的退路。
fn write_backup(dir: &Path, todo_file: &Path, stamp: &str) -> Result<PathBuf, String> {
    let target = dir.join(stamp);
    fs::create_dir_all(&target).map_err(|e| format!("建备份目录失败 {}: {}", target.display(), e))?;

    fs::copy(todo_file, target.join(F_TODO))
        .map_err(|e| format!("备份清单失败 {}: {}", target.display(), e))?;

    let cfg = paths::config_file();
    if cfg.is_file() {
        if let Err(e) = fs::copy(&cfg, target.join(F_CONFIG)) {
            log::warn!("[备份] 配置没存进备份（不影响清单）: {}", e);
        }
    }
    copy_wallpaper_in(&target);
    copy_money_in(&target);
    copy_focus_in(&target);
    Ok(target)
}

/// 把记账数据拷进备份文件夹（还没有记账数据就跳过）。失败只记日志。
///
/// 记账虽然不像清单那样三方共写，但它**只有本应用这一份**，删了就没有了 ——
/// 所以一样要进备份。老规矩：备份里带的东西，丢了都能回来。
fn copy_money_in(target: &Path) {
    let src = paths::money_file();
    if !src.is_file() {
        return;
    }
    if let Err(e) = fs::copy(&src, target.join(F_MONEY)) {
        log::warn!("[备份] 记账没拷进去（不影响清单备份）: {}", e);
    }
}

/// 把专注记录拷进备份文件夹（还没专注过就跳过）。失败只记日志。
///
/// 与记账同一条理由：虽然不像清单那样三方共写，但它**只有本应用这一份**，
/// 删了就没有了 —— 备份里带的东西，丢了都能回来。
fn copy_focus_in(target: &Path) {
    let src = paths::focus_file();
    if !src.is_file() {
        return;
    }
    if let Err(e) = fs::copy(&src, target.join(F_FOCUS)) {
        log::warn!("[备份] 专注记录没拷进去（不影响清单备份）: {}", e);
    }
}

/// 每日备份：当天已备过就跳过。返回备份路径（跳过则 None）。
pub fn backup_daily(todo_file: &Path, keep: u32) -> Result<Option<String>, String> {
    if !todo_file.is_file() {
        return Ok(None);
    }
    let dir = paths::backup_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败 {}: {}", dir.display(), e))?;

    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let target = dir.join(&day);
    if target.exists() {
        return Ok(None);
    }
    let made = write_backup(&dir, todo_file, &day)?;
    prune(&dir, keep);
    log::info!("[备份] 已备份: {}", made.display());
    Ok(Some(made.to_string_lossy().into_owned()))
}

/// 立即备份（设置页按钮）：当天已有则加时间后缀，不覆盖今天那份
pub fn backup_now(todo_file: &Path, keep: u32) -> Result<String, String> {
    if !todo_file.is_file() {
        return Err("待办文件不存在，没有可备份的内容".to_string());
    }
    let dir = paths::backup_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败 {}: {}", dir.display(), e))?;

    let now = chrono::Local::now();
    let day = now.format("%Y-%m-%d").to_string();
    let mut stamp = day.clone();
    if dir.join(&stamp).exists() {
        stamp = now.format("%Y-%m-%d-%H%M%S").to_string();
    }
    let made = write_backup(&dir, todo_file, &stamp)?;
    prune(&dir, keep);
    log::info!("[备份] 已备份: {}", made.display());
    Ok(made.to_string_lossy().into_owned())
}

// ── 列表 ──────────────────────────────────────────────────────────────────

/// 一条备份的元信息。前端要显示"时间 + 大小"，所以直接给结构化的，
/// 别让前端拿着路径自己再解析一遍。
#[derive(Debug, Clone, Serialize)]
pub struct BackupEntry {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// 本地时间 `YYYY-MM-DD HH:mm`，前端直接显示
    pub mtime: String,
    /// 这份备份里**有没有设置**（`config.json`）。老的单文件备份没有
    pub has_config: bool,
    /// 有没有带壁纸
    pub has_wallpaper: bool,
    /// 有没有带记账数据
    pub has_money: bool,
    /// 有没有带专注记录
    pub has_focus: bool,
    /// 老格式（平铺单文件）。界面上可以标一下，让用户知道它只有清单
    pub legacy: bool,
}

/// 目录总大小（备份是文件夹，大小得递归算）
fn dir_size(p: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(rd) = fs::read_dir(p) else {
        return 0;
    };
    for e in rd.flatten() {
        let path = e.path();
        match e.metadata() {
            Ok(m) if m.is_dir() => total += dir_size(&path),
            Ok(m) => total += m.len(),
            Err(_) => {}
        }
    }
    total
}

/// 列出备份（新的在前），带大小与时间
pub fn entries(limit: usize) -> Vec<BackupEntry> {
    let mut files: Vec<PathBuf> = match fs::read_dir(paths::backup_dir()) {
        Ok(rd) => rd.flatten().map(|e| e.path()).filter(|p| is_backup_entry(p)).collect(),
        Err(_) => Vec::new(),
    };
    // 名字前 10 位是日期，字典序 = 时间序
    files.sort();
    files.reverse();
    files
        .into_iter()
        .take(limit)
        .map(|p| {
            let is_dir = p.is_dir();
            let meta = fs::metadata(&p).ok();
            let size = if is_dir {
                dir_size(&p)
            } else {
                meta.as_ref().map(|m| m.len()).unwrap_or(0)
            };
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M").to_string()
                })
                .unwrap_or_default();
            BackupEntry {
                name: p
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                has_config: is_dir && p.join(F_CONFIG).is_file(),
                has_wallpaper: is_dir && p.join(F_WALLPAPER).is_dir(),
                has_money: is_dir && p.join(F_MONEY).is_file(),
                has_focus: is_dir && p.join(F_FOCUS).is_file(),
                legacy: !is_dir,
                path: p.to_string_lossy().into_owned(),
                size,
                mtime,
            }
        })
        .collect()
}

/// 列出备份路径（老接口，保留给不用元信息的地方）
pub fn list(limit: usize) -> Vec<String> {
    entries(limit).into_iter().map(|e| e.path).collect()
}

// ── 恢复 ──────────────────────────────────────────────────────────────────

/// 恢复的结果：前端要拿它拼提示，所以这些都得给
#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    /// 从哪份备份恢复的（文件名 / 文件夹名）
    pub from: String,
    /// 恢复了多少条（从 JSON 的 items 长度读）
    pub count: usize,
    /// 恢复前自动做的保险备份（用户后悔时的退路）
    pub safety: String,
    /// 这份备份里**有没有**设置
    pub has_config: bool,
    /// 这次有没有真的把设置也恢复了
    pub config_restored: bool,
    /// 这份备份里有没有壁纸
    pub has_wallpaper: bool,
    /// 这次有没有把壁纸恢复过来
    pub wallpaper_restored: bool,
    /// 这份备份里有没有记账数据
    pub has_money: bool,
    /// 这次有没有把记账数据恢复过来
    pub money_restored: bool,
    /// 这份备份里有没有专注记录
    pub has_focus: bool,
    /// 这次有没有把专注记录恢复过来
    pub focus_restored: bool,
}

/// 把配置快照恢复回去，**但保留本机字段**（见 `MACHINE_SPECIFIC_KEYS`）。
///
/// 不整体覆盖的理由：配置里混着两类东西 ——「行为设置」（主题、外观、提醒规则）
/// 和「本机路径」（清单在哪、产出根在哪、预览器装在哪）。前者跟着备份走是对的，
/// 后者跟着走就是事故：备份来自另一台机器时，那些路径根本不存在。
///
/// `wallpaper_override`：壁纸刚被恢复到新位置时，把新路径塞进去 ——
/// 否则这个字段会被本机值覆盖，恢复过来的图用不上。
fn restore_config(snap: &Path, wallpaper_override: Option<String>) -> Result<(), String> {
    let snap_text = crate::fs::read_text(snap)?;
    let mut parsed: serde_json::Value =
        serde_json::from_str(&snap_text).map_err(|e| format!("快照不是合法 JSON：{}", e))?;
    let restored = parsed
        .as_object_mut()
        .ok_or_else(|| "快照不是 JSON 对象".to_string())?;

    // 当前配置 = 本机字段的真源。读不到就当空 —— 那就把这些键全删掉，
    // 让应用的默认值接管；也好过把备份里那份外来路径写进来
    let current_text = crate::fs::read_text(&paths::config_file()).unwrap_or_default();
    let current: serde_json::Value = serde_json::from_str(&current_text).unwrap_or_default();

    for key in MACHINE_SPECIFIC_KEYS {
        match current.get(key) {
            Some(v) => {
                restored.insert(key.to_string(), v.clone());
            }
            None => {
                restored.remove(key);
            }
        }
    }
    // 壁纸例外：刚恢复过来的图在**当前**数据根里，路径得用新的
    if let Some(p) = wallpaper_override {
        restored.insert("wallpaper_path".to_string(), serde_json::Value::String(p));
    }

    let out = serde_json::to_string_pretty(&restored).map_err(|e| e.to_string())?;
    crate::fs::atomic_write(&paths::config_file(), out.as_bytes())
}

/// 把备份里的壁纸拷进当前数据根的 `壁纸/`，返回新路径。没带壁纸就返回 None。
///
/// **必须拷到当前数据根**，不能直接沿用备份目录里的那份 —— 备份是会被轮换删掉的
/// （`prune` 按份数清理），指着它当壁纸用，哪天备份被清了图就没了。
fn restore_wallpaper(backup_dir: &Path) -> Option<String> {
    let src_dir = backup_dir.join(F_WALLPAPER);
    let mut found: Option<PathBuf> = None;
    if let Ok(rd) = fs::read_dir(&src_dir) {
        for e in rd.flatten() {
            if e.path().is_file() {
                found = Some(e.path());
                break;
            }
        }
    }
    let src = found?;
    let dst_dir = paths::wallpaper_dir();
    fs::create_dir_all(&dst_dir).ok()?;
    let name = src.file_name()?.to_owned();
    let dst = dst_dir.join(name);
    // 目标已存在（恢复同一份备份两次）就不用再拷
    if !dst.is_file() {
        fs::copy(&src, &dst).ok()?;
    }
    Some(dst.to_string_lossy().into_owned())
}

/// 从某份备份恢复。**破坏性操作**，所以几道校验一道都不能省。
///
/// `with_config`：这份备份若带设置，是否一并恢复（本机路径字段不受影响）。
/// 支持两种备份：新的文件夹，和老的平铺单文件（后者只能恢复清单）。
pub fn restore(
    backup_path: &str,
    todo_file: &Path,
    keep: u32,
    with_config: bool,
) -> Result<RestoreReport, String> {
    let dir = paths::backup_dir();
    let src = PathBuf::from(backup_path);

    // ① 只认备份目录里的东西。
    // 前端是"传个路径进来"的，不卡这一道，传任何路径都能把待办覆盖成别的东西。
    let real_dir = fs::canonicalize(&dir).map_err(|e| format!("备份目录不存在: {}", e))?;
    let real_src = fs::canonicalize(&src).map_err(|e| format!("这份备份不在了: {}", e))?;
    if !real_src.starts_with(&real_dir) {
        log::warn!("[恢复] 拒绝：路径不在备份目录内 {}", real_src.display());
        return Err("这个路径不在备份目录里，已拒绝".to_string());
    }

    let legacy = real_src.is_file();
    // 清单文件：新结构在文件夹里的 `待办.json`；老格式就是文件本身
    let todo_src = if legacy { real_src.clone() } else { real_src.join(F_TODO) };

    // ② 内容必须是合法 JSON 且 items 是数组。
    // 这是恢复唯一的一道内容关口 —— 放一份坏文件上去，下次加载整份清单就打不开了。
    let text = crate::fs::read_text(&todo_src)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("这份备份不是合法 JSON：{}", e))?;
    let count = parsed
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "这份备份里没有 items 数组，不像待办文件".to_string())?
        .len();

    // ③ 先给"当前这份"留退路。恢复是破坏性操作，用户后悔时得能回去。
    let safety = backup_now(todo_file, keep)?;

    // ④ 原子写 + 登记指纹（不登记的话文件监听会把这次覆盖当成外部改动，白重载一轮）
    crate::fs::atomic_write(todo_file, text.as_bytes())?;
    todo_io::note_write(&text);

    let cfg_src = real_src.join(F_CONFIG);
    let has_config = !legacy && cfg_src.is_file();
    let has_wallpaper = !legacy && real_src.join(F_WALLPAPER).is_dir();
    let has_money = !legacy && real_src.join(F_MONEY).is_file();
    let has_focus = !legacy && real_src.join(F_FOCUS).is_file();

    // ⑤ 壁纸先落位（设置里要写它的新路径）
    let mut wallpaper_restored = false;
    let wallpaper_new = if has_wallpaper && with_config {
        match restore_wallpaper(&real_src) {
            Some(p) => {
                wallpaper_restored = true;
                Some(p)
            }
            None => None,
        }
    } else {
        None
    };

    // ⑥ 配置。**放在清单恢复成功之后** —— 清单是主角，它成了才有必要谈设置
    let mut config_restored = false;
    if with_config && has_config {
        match restore_config(&cfg_src, wallpaper_new) {
            Ok(_) => {
                config_restored = true;
                log::info!("[恢复] 设置也已恢复（路径类字段保持本机值）");
            }
            Err(e) => {
                // **设置没恢复成，不算整次失败** —— 清单已经回来了，那才是要紧的。
                // 但也不能默默吞掉：日志留痕，返回值里也带出去让界面说一句
                log::warn!("[恢复] 设置没能恢复：{}", e);
            }
        }
    }

    // ⑦ 记账数据：同样**尽力而为**。记账只有本应用这一份，但清单才是主角
    let mut money_restored = false;
    if with_config && has_money {
        match restore_money(&real_src.join(F_MONEY)) {
            Ok(_) => {
                money_restored = true;
                log::info!("[恢复] 记账数据也已恢复");
            }
            Err(e) => {
                log::warn!("[恢复] 记账数据没能恢复：{}", e);
            }
        }
    }

    // ⑧ 专注记录：同样**尽力而为**（理由同记账）。只有本应用写它，没有监听，
    //    所以不必像记账那样再登记指纹
    let mut focus_restored = false;
    if with_config && has_focus {
        match restore_focus(&real_src.join(F_FOCUS)) {
            Ok(_) => {
                focus_restored = true;
                log::info!("[恢复] 专注记录也已恢复");
            }
            Err(e) => {
                log::warn!("[恢复] 专注记录没能恢复：{}", e);
            }
        }
    }

    let from = real_src
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| backup_path.to_string());
    log::info!("[恢复] 已从 {} 恢复 {} 条，保险备份 {}", from, count, safety);
    Ok(RestoreReport {
        from,
        count,
        safety,
        has_config,
        config_restored,
        has_wallpaper,
        wallpaper_restored,
        has_money,
        money_restored,
        has_focus,
        focus_restored,
    })
}

/// 把备份里的记账数据写回数据根。
///
/// **先校验 JSON 合法再落盘**：放一份坏文件上去，下次打开记账界面就是空白，
/// 用户会以为自己的账目丢了 —— 这道关口和清单恢复那道是同一个道理。
fn restore_money(snap: &Path) -> Result<(), String> {
    let text = crate::fs::read_text(snap)?;
    let _: crate::money::MoneyData =
        serde_json::from_str(&text).map_err(|e| format!("备份里的记账数据不是合法 JSON：{}", e))?;
    crate::fs::atomic_write(&paths::money_file(), text.as_bytes())?;
    // 登记的语义是"这份是本应用刚写的"：恢复是刻意的整份覆盖，
    // 不登记的话文件监听会把它当成外部改动，又通知前端重载一轮
    crate::money::note_write(&text);
    Ok(())
}

/// 把备份里的专注记录写回数据根。
///
/// **先校验 JSON 合法再落盘**（同 `restore_money`）：放一份坏文件上去，
/// 统计里就一片空白，而用户会以为"我的专注记录丢了" —— 这道关口和清单那道是一个道理。
fn restore_focus(snap: &Path) -> Result<(), String> {
    let text = crate::fs::read_text(snap)?;
    let _: crate::focus_log::FocusData = serde_json::from_str(&text)
        .map_err(|e| format!("备份里的专注记录不是合法 JSON：{}", e))?;
    crate::fs::atomic_write(&paths::focus_file(), text.as_bytes())?;
    // 刻意**不**登记指纹：登记那套只服务于"文件监听过滤自己的回声"，
    // 而专注记录不在 watcher 的监听名单里（只有本应用会写它）
    Ok(())
}

// ── 清理 ──────────────────────────────────────────────────────────────────

/// 只保留最近 keep 份，其余删掉
fn prune(dir: &Path, keep: u32) {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).filter(|p| is_backup_entry(p)).collect(),
        Err(_) => return,
    };
    if files.len() <= keep as usize {
        return;
    }
    files.sort();
    let drop = files.len() - keep as usize;
    for p in files.into_iter().take(drop) {
        let r = if p.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
        if let Err(e) = r {
            log::warn!("[备份] 清理旧备份失败 {}: {}", p.display(), e);
        }
    }
}
