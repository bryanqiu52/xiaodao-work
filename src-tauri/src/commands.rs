//! 前端 invoke 入口。
//!
//! 约定：**命令里不做业务判断**，只做"拿配置 → 调对应模块 → 回结果"。
//! 待办的语义（完成态自洽、软删除、流水、审批）全在前端 `src/core/`。

use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::backup;
use crate::config::{self, AppConfig};
use crate::edge;
use crate::github;
use crate::open;
use crate::paths;
use crate::shortcut;
use crate::todo_io::{self, TodoSnapshot, WriteResult};
use crate::tray;

// ── 待办文件 ──────────────────────────────────────────────────────────────

/// 读待办文件原文（前端自己 JSON.parse）
///
/// 路径**不看配置**：待办文件固定是 `数据根\待办.json`（见 `paths::todo_file`）
#[tauri::command]
pub fn todo_read() -> Result<TodoSnapshot, String> {
    todo_io::read_todo(&paths::todo_file())
}

/// 写待办文件：必须带上这次改动所基于的指纹，盘上不是这一份就拒绝
#[tauri::command]
pub fn todo_write(text: String, base_fingerprint: String) -> Result<WriteResult, String> {
    todo_io::write_todo(&paths::todo_file(), &text, &base_fingerprint)
}

// ── 配置 ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn config_get() -> AppConfig {
    config::load()
}

#[tauri::command]
pub fn config_set(mut cfg: AppConfig) -> Result<AppConfig, String> {
    // 字号百分比可能被手改成离谱的值，落盘前夹回安全区间（前端已夹过，这里是兜底）
    cfg.ui_scale = cfg.ui_scale.clamp(config::UI_SCALE_MIN, config::UI_SCALE_MAX)
        / 5
        * 5;
    // 贴边隐藏的开关要在内存里同步一份，不然要等下次启动才生效
    edge::set_enabled(cfg.edge_hide);
    let _guard = config::lock();
    // **`window` 一律保留盘上现值，不信前端传来的**。
    // 这个字段只有后端在写（lib.rs 的 `save_window_state`，拖窗口时节流落盘）；
    // 前端手里那份是启动时读的、之后永远不更新。前端是整份传回来的，
    // 照单全收就会把后端刚记下的尺寸冲回旧值 —— 用户拖完窗口随手改个设置，
    // 下次启动窗口就跳回旧位置。到期提醒还会自动写配置（跨天 / 记已提醒），
    // 用户不动手也会撞上，所以这道防线必须设在后端。
    cfg.window = config::load().window;
    config::save(&cfg).map(|_| cfg)
}

// ── 备份 ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn backup_now() -> Result<String, String> {
    let cfg = config::load();
    backup::backup_now(&paths::todo_file(), cfg.backup_keep)
}

#[tauri::command]
pub fn backup_list() -> Vec<String> {
    backup::list(30)
}

// ── 路径 ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn data_root_path() -> String {
    paths::data_root_string()
}

/// 当前数据根是不是用户自己选的（不是默认、也不是便携模式）。
/// 前端拿它决定要不要显示「恢复默认」——没自定义过就没得恢复。
#[tauri::command]
pub fn data_root_custom() -> Option<String> {
    paths::custom_root().map(|p| p.to_string_lossy().into_owned())
}

/// 换数据根：先把该搬的搬过去，**成功之后才记下新路径**。
/// 返回一句给用户看的话；失败把原因原样带回（界面上要说得出来，不许静默）。
#[tauri::command]
pub fn data_root_set(path: String) -> Result<String, String> {
    let p = std::path::PathBuf::from(path.trim());
    if p.as_os_str().is_empty() {
        return Err("没选到目录".to_string());
    }
    let msg = paths::migrate_to(&p)?;
    log::info!("[数据] {}", msg);
    Ok(msg)
}

/// 恢复默认数据根。
/// **不搬东西** —— 默认目录里原来那份还在（搬家时旧目录一直保留着），切回去就行。
#[tauri::command]
pub fn data_root_reset() -> Result<String, String> {
    paths::clear_custom_root()?;
    let msg = format!(
        "已恢复默认数据目录 {}。重启后生效",
        paths::default_data_root().display()
    );
    log::info!("[数据] {}", msg);
    Ok(msg)
}

/// 清空**所有数据**，回到刚装好的状态。
///
/// 这是全应用唯一不可逆的操作，所以三条规矩：
///   ① **先自动备份一份** —— 备份失败就不往下走，绝不给"清空完才发现没备份"的机会；
///   ② **刻意覆盖，不走指纹比对** —— 重置本来就是要盖掉，让指纹拦着反而动不了；
///      但写完要 `note_write` 登记一下，免得文件监听把它当成"AI 改的"又通知前端重载一轮；
///   ③ **备份目录不动** —— 那是唯一的后悔药，连它一起清就真没了。
///
/// 返回一句给用户看的话：清掉了哪些、备份留在哪。
#[tauri::command]
pub fn data_reset() -> Result<String, String> {
    let cfg = config::load();
    let mut done: Vec<String> = Vec::new();

    // ① 先备份（得有待办文件才谈得上备份）
    let todo = paths::todo_file();
    if todo.is_file() {
        backup::backup_now(&todo, cfg.backup_keep).map_err(|e| {
            format!("想先备份一份再清空，但备份失败了（{}）。没有备份就不清了", e)
        })?;
    }

    // ② 待办：保留文件里可能还有的其它字段，只把 items 清成空
    let raw = crate::fs::read_text(&todo).unwrap_or_default();
    let mut doc: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = doc.as_object_mut() {
        obj.insert("version".into(), serde_json::json!(1));
        obj.insert("items".into(), serde_json::json!([]));
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|e| format!("生成空清单失败: {}", e))?;
    crate::fs::atomic_write(&todo, text.as_bytes())?;
    todo_io::note_write(&text);
    done.push("待办清单".to_string());

    // ③ 记账：整份写空
    let money_text = "{\n  \"version\": 1,\n  \"entries\": []\n}";
    crate::fs::atomic_write(&paths::money_file(), money_text.as_bytes())?;
    crate::money::note_write(money_text);
    done.push("记账".to_string());

    // ③-2 专注记录：同样写空。
    // **必须跟着清** —— "恢复出厂设置"说好了是"所有数据"，
    // 留下专注记录会让用户以为没清干净（而它确实还在）。
    // 只有本应用一个写入方，所以不必像记账那样再登记指纹。
    crate::focus_log::clear()?;
    done.push("专注记录".to_string());

    // ④ 设置恢复默认。待办文件位置也一起回到默认 —— 说好了是"所有数据"
    config::save(&AppConfig::default())?;
    done.push("设置".to_string());

    // ⑤ 壁纸：拷进来的那些图也清掉。
    //    日志不动 —— 正在写它，删了也没意义，还会让这次操作留下一个说不清的失败
    let wp = paths::wallpaper_dir();
    if wp.is_dir() {
        cleanup_wallpapers();
        done.push("壁纸".to_string());
    }

    log::info!("[数据] 已清空：{}", done.join("、"));
    Ok(format!(
        "已清空：{}。清空前自动备份了一份，后悔了可以在上面「从备份恢复」里翻回来。",
        done.join("、")
    ))
}

/// 打开产出 / 目录。返回值见 `open.rs` 的说明
#[tauri::command]
pub fn open_path(path: String, mode: Option<String>) -> String {
    open::open_path(&path, mode.as_deref().unwrap_or("file"))
}

// ── 系统对话框 ────────────────────────────────────────────────────────────

/// 选目录（设置页白名单用）；取消返回 null
#[tauri::command]
pub async fn pick_directory(app: AppHandle, current: Option<String>) -> Option<String> {
    let cur = current.unwrap_or_default();
    let mut dialog = app.dialog().file();
    if !cur.is_empty() {
        dialog = dialog.set_directory(cur);
    }
    // FilePath 不是 PathBuf（可能是 URL 形态），先转成字符串再还原成路径
    dialog
        .blocking_pick_folder()
        .map(|p| simplify(&p.to_string()))
}

/// 选文件（设置页选 待办.json / MD-Preview.exe 用）；取消返回 null
#[tauri::command]
pub async fn pick_file(app: AppHandle, filters: Option<Vec<String>>) -> Option<String> {
    let exts = filters.unwrap_or_default();
    let mut dialog = app.dialog().file();
    if !exts.is_empty() {
        let owned: Vec<String> = exts.clone();
        let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter("文件", &refs);
    }
    dialog.blocking_pick_file().map(|p| simplify(&p.to_string()))
}

/// 剥掉 `\\?\` 前缀再回给前端（交给 explorer 和展示给用户时都不能带）
fn simplify(s: &str) -> String {
    paths::simplify_path(std::path::Path::new(s))
        .to_string_lossy()
        .into_owned()
}

// ── 窗口控制 ──────────────────────────────────────────────────────────────
//
// **为什么不直接用前端自带的 window API**：那条路一旦失败是静默的 ——
// 无边框窗口没有 DevTools 可看，表现就成了"按钮按了没反应"，排查全靠猜。
// 改走 invoke 之后，失败会带回错误串，前端能显示出来，日志里也留痕。

/// 主窗口。`"main"` 是 tauri.conf.json 里不写 label 时的默认名
fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())
}

#[tauri::command]
pub fn win_minimize(app: AppHandle) -> Result<(), String> {
    main_window(&app)?.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn win_toggle_maximize(app: AppHandle) -> Result<(), String> {
    let win = main_window(&app)?;
    // 贴边收起时窗口大半在屏幕外，先滑回来 —— 否则最大化会把那条缝也算进尺寸
    if edge::is_hidden() {
        // 点标题栏按钮叫出来的：鼠标就在窗口上，不用再等它进来一次（false）
        edge::reveal(&app, false);
    }
    if win.is_maximized().unwrap_or(false) {
        win.unmaximize().map_err(|e| e.to_string())
    } else {
        win.maximize().map_err(|e| e.to_string())
    }
}

/// 关窗口：按设置决定是藏到托盘还是退出
#[tauri::command]
pub fn win_close(app: AppHandle) -> Result<(), String> {
    if config::load().close_behavior == "quit" {
        app.exit(0);
        return Ok(())
    }
    let win = main_window(&app)?;
    win.hide().map_err(|e| e.to_string())
}

/// 置顶开关：窗口真的置顶/取消，**同时**把开关状态落进配置
#[tauri::command]
pub fn win_set_always_on_top(app: AppHandle, value: bool) -> Result<(), String> {
    let win = main_window(&app)?;
    win.set_always_on_top(value).map_err(|e| e.to_string())?;
    let _guard = config::lock();
    let mut cfg = config::load();
    cfg.always_on_top = value;
    config::save(&cfg).map_err(|e| e.to_string())
}

/// 标题栏按下鼠标 → 开始拖窗口
#[tauri::command]
pub fn win_start_drag(app: AppHandle) -> Result<(), String> {
    main_window(&app)?
        .start_dragging()
        .map_err(|e| e.to_string())
}

// ── 诊断 ──────────────────────────────────────────────────────────────────

/// 前端日志桥接：Windows 的 WebView 没地方看 console，
/// 前端的报错从这儿进 Rust 日志，`数据根\logs\` 里就能翻到。
#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "warn" => log::warn!("[前端] {}", message),
        "info" => log::info!("[前端] {}", message),
        _ => log::error!("[前端] {}", message),
    }
}

/// 自检：证明"前端确实连上了桌面"，顺带把运行时环境一起打包回来
#[tauri::command]
pub fn env_probe(app: AppHandle) -> String {
    let version = app.package_info().version.to_string();
    let scale = main_window(&app)
        .and_then(|w| w.scale_factor().map_err(|e| e.to_string()))
        .map(|s| format!("{:.2}", s))
        .unwrap_or_else(|e| format!("读不到({})", e));
    format!(
        "version={} debug={} data_root={} scale={}",
        version,
        cfg!(debug_assertions),
        paths::data_root_string(),
        scale
    )
}

// ── 快捷键与窗口 ──────────────────────────────────────────────────────────

/// 改全局快捷键。返回 `ok` / `conflict` / `invalid:...`
#[tauri::command]
pub fn set_shortcut(app: AppHandle, shortcut: String) -> String {
    match shortcut::change(&app, &shortcut) {
        Ok(()) => {
            // 成功才落配置，失败时配置里仍是旧的（和已注册的保持一致）
            let _guard = config::lock();
            let mut cfg = config::load();
            cfg.global_shortcut = shortcut;
            let _ = config::save(&cfg);
            "ok".to_string()
        }
        Err(e) => e,
    }
}

/// 把用户选的壁纸图**拷进数据根**，返回拷好之后的绝对路径。
///
/// **为什么要拷一份，而不是存原路径**：用户随时会把原图挪走、改名、删掉 ——
/// 只存原路径的话，某天壁纸突然"不见了"，而他根本想不到是自己挪了原图。
/// 拷一份进数据根之后，配置里存的是"我们自己的文件"，稳。
#[tauri::command]
pub fn import_wallpaper(source: String, builtin_name: Option<String>) -> Result<String, String> {
    let src = std::path::PathBuf::from(&source);
    if !src.is_file() {
        return Err("这个文件不在了（可能已移动或改名）".to_string());
    }
    let dir = paths::wallpaper_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建壁纸目录失败: {}", e))?;

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_lowercase();
    // 内置壁纸用**固定文件名**（`内置-<图名>`）：界面靠它判断"卡片是不是选中态"，
    // 重选同一张直接覆盖，也不会在目录里堆一堆同名副本。
    // 自选的图仍然带时间戳：换一张新图不会覆盖旧图（旧图还在，想换回去随时能翻到）
    let name = match builtin_name {
        Some(n) if !n.trim().is_empty() => format!("内置-{}.{}", n.trim(), ext),
        _ => format!("壁纸-{}.{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), ext),
    };
    let dst = dir.join(name);
    std::fs::copy(&src, &dst).map_err(|e| format!("拷贝壁纸失败: {}", e))?;

    let out = paths::simplify_path(&dst).to_string_lossy().into_owned();
    log::info!("[壁纸] 已导入: {}", out);
    Ok(out)
}

/// 清空**整个**壁纸目录。**只给「恢复出厂设置」用** —— 那个场景要的就是把
/// 本应用自己的东西全部清掉，整目录删才是对的。
///
/// 设置里那个「移除」按钮走 `remove_wallpaper`（只删当前这一张）：
/// 2026-09-25 之前「移除」调的是这个函数，结果用户自己放进壁纸目录的图跟着一起没了。
pub(crate) fn cleanup_wallpapers() {
    let dir = paths::wallpaper_dir();
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => log::info!("[壁纸] 已清空壁纸目录"),
        // 目录本来就不在 = 没什么可清的，不算错
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("[壁纸] 清理壁纸目录失败 {}: {}", dir.display(), e),
    }
}

/// 只删**当前这一张**壁纸。设置里点「移除」时调用。
///
/// **必须校验它在壁纸目录里**：这个命令收的是前端给的路径，不校验就成了
/// "前端可以指定删任意一个文件"。配置被改坏或将来有别处调用时都会踩到，
/// 所以两边都 `canonicalize` 解成真实路径再比 —— `..\` 这类绕路写法也挡得住。
///
/// 文件已经不在（手删过、换过数据目录）算成功：界面要的是"这张不用了"，不在了正合适。
#[tauri::command]
pub fn remove_wallpaper(path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path);
    if target.as_os_str().is_empty() {
        return Ok(());
    }
    let dir = paths::wallpaper_dir();
    // 目录还没建（从没选过壁纸）时解不出来：那种情况下也没有文件可删
    let inside = match (target.canonicalize(), dir.canonicalize()) {
        (Ok(t), Ok(d)) => t.starts_with(&d),
        _ => false,
    };
    if !inside {
        log::warn!("[壁纸] 要删的不在壁纸目录里，已拒绝: {}", path);
        return Err("这张图不在壁纸目录里，没删".to_string());
    }
    match std::fs::remove_file(&target) {
        Ok(()) => {
            log::info!("[壁纸] 已移除: {}", path);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("删不掉这张图: {}", e)),
    }
}

/// 一条内置壁纸（资源目录 `Wallpaper` 里的图片）。
#[derive(serde::Serialize)]
pub struct BuiltinWallpaper {
    /// 文件名（去掉扩展名），卡片上显示用
    pub name: String,
    /// 绝对路径。只用于两处：显示缩略图（前端 convertFileSrc）、
    /// 以及选中后传给 `import_wallpaper` 拷进数据根
    pub path: String,
}

/// 列出内置壁纸。
///
/// **只列不拷**：挑中哪一张仍走 `import_wallpaper` 拷进数据根，
/// 这样备份 / 恢复 / 清空那套逻辑完全不用知道"还有个图库"。
/// 目录不在或读不出来就返回空列表 —— 界面据此显示"把图片放进 Wallpaper 文件夹"，
/// 比"点了没反应"强。
#[tauri::command]
pub fn builtin_wallpapers(app: AppHandle) -> Vec<BuiltinWallpaper> {
    // 认得的图片扩展名：跟前端选图时的 filters 保持一致
    const EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif"];

    let Some(dir) = paths::builtin_wallpaper_dir(&app) else {
        log::warn!("[壁纸] 取不到资源目录，读不了内置壁纸");
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        // 目录不存在不算错：用户可能还没往里放图，界面会给提示
        log::info!("[壁纸] 内置壁纸目录不在（还没放图？）: {}", dir.display());
        return Vec::new();
    };

    let mut out: Vec<BuiltinWallpaper> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !EXTS.contains(&ext.as_str()) {
            continue;
        }
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(BuiltinWallpaper {
            name,
            path: paths::simplify_path(&p).to_string_lossy().into_owned(),
        });
    }
    // 按文件名排：目录遍历的顺序是文件系统说了算的，不加排序每次打开都可能换位置
    out.sort_by(|a, b| a.name.cmp(&b.name));
    log::info!("[壁纸] 内置壁纸 {} 张（{}）", out.len(), dir.display());
    out
}

/// 用默认浏览器打开一个网址。
///
/// **只卡协议，不卡域名**：以前只放行自家域名，结果自家官网还因为参数名对不上
/// 打不开（2026-09-25 修）。域名白名单挡不住真正危险的东西 —— 危险的是协议
/// （`file://`、自定义协议能拉起别的程序），所以这里只放行 http / https。
///
/// 它不走 `open_path` —— 那条路先判文件存不存在、再过目录白名单，
/// 网址会被当成"文件不在了"挡掉。
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if !is_http_url(url) {
        log::warn!("[打开] 只认 http / https，已拒绝: {}", url);
        return Err("只认 http / https 开头的网址".to_string());
    }
    opener::open(url).map_err(|e| format!("打开浏览器失败: {}", e))
}

/// 是不是 http / https 网址。**协议之外一律拒绝** —— 别让 `file://`、
/// 自定义协议借这个口子去拉起别的程序。
fn is_http_url(raw: &str) -> bool {
    let Some((scheme, rest)) = raw.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    // 光有协议不够：`https://` 后面得真有个 host，不能是空串或只剩路径
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty()
}

/// 当前是不是便携模式（数据跟着 exe 走，放在 `exe目录\data\`）
#[tauri::command]
pub fn is_portable() -> bool {
    paths::is_portable()
}

/// 发一条系统通知（到期提醒用）。
///
/// **为什么由 Rust 发而不是前端**：前端的 plugin-notification 要过 ACL 授权，
/// 而这条链路上任何一次静默失败都意味着"提醒没来，用户还以为自己没到期"。
/// 走 Rust 少一层授权，失败还能带着原因回给前端。
#[tauri::command]
pub fn notify(app: AppHandle, title: String, body: String) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| {
            log::warn!("[提醒] 发送系统通知失败: {}", e);
            e.to_string()
        })
}

/// 列出备份（带大小与时间，新的在前）
#[tauri::command]
pub fn backup_entries() -> Vec<backup::BackupEntry> {
    backup::entries(60)
}

/// 从某份备份恢复待办清单（可选一并恢复配置）。
///
/// **破坏性操作**：restore 内部会先把当前这份备份掉，再覆盖。失败原因回给界面显示。
/// `with_config` 只在"这份备份确实带配置快照"时起作用；恢复配置时本机路径字段会保留当前值。
#[tauri::command]
pub fn backup_restore(
    path: String,
    with_config: bool,
) -> Result<backup::RestoreReport, String> {
    let cfg = config::load();
    backup::restore(
        &path,
        &paths::todo_file(),
        cfg.backup_keep,
        with_config,
    )
}

/// GitHub 热榜：按语言与时间范围拉热门仓库（Rust 侧缓存 10 分钟）。
#[tauri::command]
pub async fn github_trending(lang: String, since: String) -> Result<github::TrendingResp, String> {
    github::trending(&lang, &since).await
}

/// 用系统浏览器打开仓库页（只放行 GitHub 域，校验在 github.rs）。
#[tauri::command]
pub fn github_open(url: String) -> Result<(), String> {
    github::open_repo(&url)
}

/// 读记账数据（带内容指纹，写回时要原样带回来）
#[tauri::command]
pub fn money_read() -> crate::money::MoneySnapshot {
    crate::money::read_snapshot()
}

/// 全量保存记账数据。
///
/// `base_fingerprint` 是这次改动**所基于**的那份指纹：盘上已经不是那一版就拒绝写入，
/// 把最新的回传（`latest`）—— 记账现在有 AI 这个第二个写入方，少了比对就会互相覆盖
/// （AI 记一笔，界面上随后保存一次，AI 那笔就没了）。
/// 前端持有全部流水（个人记账量级很小），整份写回最简单可靠。
#[tauri::command]
pub fn money_write(
    data: crate::money::MoneyData,
    base_fingerprint: String,
) -> Result<crate::money::MoneyWriteResult, String> {
    // 后端兜底校验：手改 JSON 塞进天文数字也不至于把汇总卡撑爆
    for e in &data.entries {
        if e.amount_cents < 0 || e.amount_cents > 10_000_000_000 {
            return Err(format!(
                "「{}」的金额离谱得不像话（{} 分），拒绝保存",
                e.title, e.amount_cents
            ));
        }
    }
    let result = crate::money::write(&data, &base_fingerprint)?;
    if result.ok {
        log::info!("[记账] 已保存 {} 条", data.entries.len());
    }
    Ok(result)
}

/// 读专注记录。文件不存在或解析失败都回空结构（不是错误，见 `focus_log.rs`）。
#[tauri::command]
pub fn focus_log_read() -> crate::focus_log::FocusData {
    crate::focus_log::read()
}

/// 追加一条专注记录。
///
/// **没有 `base_fingerprint`** —— 与记账不同，这个文件只有本应用一个写入方，
/// 指纹比对没有对象。串行化交给 `focus_log` 里那把锁。
#[tauri::command]
pub fn focus_log_append(record: crate::focus_log::FocusRecord) -> Result<(), String> {
    crate::focus_log::append(record)
}

/// 快捷键当前到底注册上了没：`ok` / `conflict` / `invalid` / `not-registered`。
///
/// **启动时注册失败不能只在日志里说**：用户只会发现"快捷键没反应"，却不知道是被人占了。
/// 设置页和启动提示都靠这个查。
#[tauri::command]
pub fn shortcut_state() -> String {
    shortcut::state()
}

#[tauri::command]
pub fn show_window(app: AppHandle) {
    tray::show_window(&app)
}

/// 鼠标碰到边上的那条缝：把收起来的窗口滑出来。
///
/// 鼠标此刻就在边上，所以**不设**"等鼠标进来一次"（false）——
/// 设了的话，鼠标一划而过时守护线程往往没看见它进过窗口，窗口就不肯再收回去。
#[tauri::command]
pub fn edge_reveal(app: AppHandle) {
    edge::reveal(&app, false);
}

/// 鼠标离开窗口一会儿了：把"鼠标还在里头"的标记撤掉（守护线程这才肯再接管），
/// 然后如果它正贴着屏幕边缘，就收起来
#[tauri::command]
pub fn edge_hide(app: AppHandle) {
    // 光标的实时位置由守护线程自己查（`edge::pointer_in_window`），这里不用报备
    edge::hide_to_edge(&app);
}
