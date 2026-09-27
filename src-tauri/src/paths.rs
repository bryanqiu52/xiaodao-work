//! 数据根目录解析。
//!
//! 数据根放配置和备份，与待办文件**分开**：
//!   - 数据根：`%APPDATA%\小刀工作台`（便携版：`exe 目录\data\`）—— 本应用自己的东西；
//!   - 待办文件：路径由配置 `todo_file` 指定（默认 = 数据根下的 `待办.json`）——
//!     它是**可以和别的程序 / AI 共用的真相源**，所以可以放到任何地方，本应用不假定它在哪。
//!
//! 这个分离很重要：数据根里的东西可以随便删改，待办文件不行。
//! 备份、日志、配置全放数据根，`待办.json` 旁边一个多余的文件都不留。
//!
//! 写并发**不用文件锁**：同进程内的并发由 `todo_io` 里的 Mutex 拦，
//! 多开由 single-instance 插件把第二个进程顶掉 —— 所以数据根里没有 `todo.lock`。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 便携标志：exe 同目录有这个文件就走便携模式
pub const PORTABLE_MARKER: &str = "portable";
/// 便携版数据子目录
pub const PORTABLE_DATA_DIR: &str = "data";

static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// 默认数据根（标准版）
pub fn default_data_root() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("小刀工作台")
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.to_path_buf())
}

/// 自定义数据根记在这个文件里。
///
/// **它必须待在固定位置**（也就是默认数据根里），不能跟着数据根走 ——
/// 否则就成了自举循环：得先读数据根，才知道数据根在哪。
pub fn location_file() -> PathBuf {
    default_data_root().join("location.txt")
}

/// 用户自定义的数据根。没设过、文件坏了、内容为空都返回 `None`（回退默认）。
pub fn custom_root() -> Option<PathBuf> {
    let text = std::fs::read_to_string(location_file()).ok()?;
    // **先削掉 BOM**：这个文件用户很可能会拿记事本打开改，而记事本默认写 UTF-8 BOM。
    // 那个 `\u{feff}` 肉眼不可见，混进路径里会让整个 PathBuf 变成另一个目录 ——
    // 而且报错方式会是"配置读不出来"这种毫不相干的症状。
    let cleaned = text.trim_start_matches('\u{feff}').trim();
    let p = PathBuf::from(cleaned);
    if p.as_os_str().is_empty() {
        None
    } else {
        Some(p)
    }
}

fn resolve_data_root() -> PathBuf {
    // 便携版的优先级最高：数据跟着 exe 走，这时候 location 一律不认 ——
    // 一个"便携"的应用却把数据写到 %APPDATA%，那就不叫便携了
    if let Some(dir) = exe_dir() {
        if dir.join(PORTABLE_MARKER).exists() {
            return dir.join(PORTABLE_DATA_DIR);
        }
    }
    if let Some(p) = custom_root() {
        return p;
    }
    default_data_root()
}

/// 递归拷目录。**不用 rename** —— 换到别的盘时 rename 会直接失败，拷才哪里都能用。
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// 换数据根之前的把关：能建、能写、不套娃。
fn check_new_root(new_root: &Path) -> Result<(), String> {
    let old = data_root();
    if new_root == old {
        return Err("新路径跟当前一样，不用改".to_string());
    }
    // 新目录套在旧目录里，拷贝时会把自己拷进自己，无限递归
    if new_root.starts_with(old) {
        return Err("新路径在当前数据目录里面，会套娃。请选一个独立的目录".to_string());
    }
    std::fs::create_dir_all(new_root).map_err(|e| format!("建不了这个目录：{}", e))?;
    // **真写一个文件试试**：只建目录不够 —— 目录可能早就存在但是只读的，
    // 那种目录 create_dir_all 会成功，之后写配置才失败
    let probe = new_root.join(".xiaodao-write-test");
    std::fs::write(&probe, b"ok").map_err(|e| format!("这个目录写不进去：{}", e))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// 把数据根换到 `new_root`，并把该搬的东西搬过去。返回一句给用户看的结果。
///
/// **搬什么、不搬什么**：
///   - 搬 `config.json`：不搬的话界面设置全丢，用户会以为"改个路径把配置搞没了"；
///   - 搬 `壁纸/`：那是用户挑的图，当初拷进数据根本来就是为了不丢；
///   - 搬 `待办备份/`：恢复功能要靠它；
///   - 搬 `记账.json` / `专注记录.json`：**数据根里的每一份数据都得跟着走** ——
///     少了它们，换完目录界面上就是一片空白（文件还躺在旧目录里），
///     用户看到的是"我的账目 / 专注记录没了"，比配置丢了还吓人；
///   - **不搬 `logs/`**：历史日志没有保留价值，而且此刻正往里写、拷出来必然是半截的。
///
/// **旧目录一律保留**：这是搬家，不是搬走 —— 后悔了还能翻回去。
/// 待办文件（`todo.json`）不在数据根里，所以它一动不动（那是三方共用的真相源）。
pub fn migrate_to(new_root: &Path) -> Result<String, String> {
    check_new_root(new_root)?;
    let old = data_root();
    let mut moved: Vec<&str> = Vec::new();

    let cfg = old.join("config.json");
    if cfg.exists() {
        std::fs::copy(&cfg, new_root.join("config.json"))
            .map_err(|e| format!("配置复制失败：{}", e))?;
        moved.push("配置");
    }

    let wp = old.join("壁纸");
    if wp.is_dir() {
        copy_dir_all(&wp, &new_root.join("壁纸")).map_err(|e| format!("壁纸复制失败：{}", e))?;
        moved.push("壁纸");
    }

    let bak = old.join("待办备份");
    if bak.is_dir() {
        copy_dir_all(&bak, &new_root.join("待办备份"))
            .map_err(|e| format!("备份复制失败：{}", e))?;
        moved.push("备份");
    }

    // 数据根里的单文件数据，一份都不能落下。
    // 判定标准很简单：**它是"数据根里的东西"，就该跟着数据根走** —— 挨个点名容易漏，
    // 所以这里只挂了两个"会丢出真损失"的（账目、专注记录），
    // 以后再加同类文件时记得同步这张表。
    for (name, label) in [("记账.json", "记账"), ("专注记录.json", "专注记录")] {
        let src = old.join(name);
        if !src.is_file() {
            continue;
        }
        std::fs::copy(&src, new_root.join(name))
            .map_err(|e| format!("{}复制失败：{}", label, e))?;
        moved.push(label);
    }

    // **先搬完再写 location**：万一中途失败，location 还是旧的，
    // 下次启动仍然读老目录，用户的数据不会"人间蒸发"
    set_custom_root(new_root)?;

    let what = if moved.is_empty() {
        "（这里是空的，没有可搬的东西）".to_string()
    } else {
        format!("，已搬过去：{}", moved.join("、"))
    };
    Ok(format!(
        "数据目录已改为 {}{}。重启后生效",
        new_root.display(),
        what
    ))
}

/// 记下自定义数据根
pub fn set_custom_root(path: &Path) -> Result<(), String> {
    let file = location_file();
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("建不了配置目录：{}", e))?;
    }
    std::fs::write(&file, path.to_string_lossy().as_bytes())
        .map_err(|e| format!("写不了路径记录：{}", e))
}

/// 恢复默认数据根（删掉那条记录）
pub fn clear_custom_root() -> Result<(), String> {
    let file = location_file();
    if file.exists() {
        std::fs::remove_file(&file).map_err(|e| format!("删不掉路径记录：{}", e))?;
    }
    Ok(())
}

/// 数据根（进程内只解析一次）
pub fn data_root() -> &'static Path {
    DATA_ROOT.get_or_init(resolve_data_root).as_path()
}

/// 数据根的可读字符串（给设置页展示）
pub fn data_root_string() -> String {
    data_root().to_string_lossy().into_owned()
}

pub fn config_file() -> PathBuf {
    data_root().join("config.json")
}

pub fn backup_dir() -> PathBuf {
    data_root().join("待办备份")
}

/// 记账数据文件。记账是本应用和 AI 两方写的（不像待办三方共写），
/// 所以放数据根、走自己的读写模块，并且**一样进备份** ——
/// 它只有本应用这一份，删了就没了（备份结构见 `backup.rs`）
pub fn money_file() -> PathBuf {
    data_root().join("记账.json")
}

/// 专注记录文件。**只有本应用一个写入方**（AI 不写它），
/// 所以读写比记账还简单一层（见 `focus_log.rs` 的注释）。
/// 一样进备份：同一条理由 —— 丢了就没有第二份。
pub fn focus_file() -> PathBuf {
    data_root().join("专注记录.json")
}

pub fn log_dir() -> PathBuf {
    data_root().join("logs")
}

/// 壁纸目录。用户选中的图会被**拷一份**进来 —— 原图挪走、改名、删掉都不影响。
pub fn wallpaper_dir() -> PathBuf {
    data_root().join("壁纸")
}

/// 内置壁纸文件夹名（随安装包分发，放在资源目录下）。
///
/// 跟上面那个"拷进来的壁纸"是两回事：这里是**图库**，用户从里面挑一张，
/// 挑完照样拷进 `wallpaper_dir()` —— 之后备份、恢复、清空都只认数据根里那一份。
pub const BUILTIN_WALLPAPER_DIR: &str = "Wallpaper";

/// 内置壁纸目录（资源目录下的 `Wallpaper`），取不到就回 `None`。
///
/// **三个场景都靠它**：Windows 上 `resource_dir()` 打包后就是 exe 所在目录
/// （安装版 = 安装目录根），dev 模式下是 `src-tauri` 目录。
/// 所以图片放 `src-tauri\Wallpaper`，dev 能直接读到；
/// 安装版靠 `bundle.resources` 带上；便携版靠打包脚本拷到 exe 旁边。
pub fn builtin_wallpaper_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    let dir = app.path().resource_dir().ok()?.join(BUILTIN_WALLPAPER_DIR);
    if dir.is_dir() {
        return Some(dir);
    }
    // dev 模式（`tauri dev`）兜一次编译期的工程目录：那才是图片真正躺着的地方。
    // 少了这一层，开发时卡片区永远是空的，很容易被当成"图没放进图库"。
    #[cfg(debug_assertions)]
    {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BUILTIN_WALLPAPER_DIR);
        if dev.is_dir() {
            return Some(dev);
        }
    }
    // 目录不在也照样回：调用方据此记日志 / 给界面提示，好过静默返回 None
    Some(dir)
}

/// 当前是不是便携模式（exe 旁边有 `portable` 标志文件）。
///
/// 注意 `data_root()` 是 `OnceLock`、只在启动时解析一次 —— 所以**运行中改不了模式**，
/// 想让便携生效必须重启（界面上也要说清楚，否则用户会以为那个标志文件没生效）。
pub fn is_portable() -> bool {
    exe_dir()
        .map(|d| d.join(PORTABLE_MARKER).exists())
        .unwrap_or(false)
}

/// 主程序旁边的 md 查看器（发布的便携版 zip 里就把 `MD-Preview.exe` 放在这）。
///
/// **为什么要有这个兜底**：配置里 `preview_exe` 的默认值是**作者本机**的工具路径，
/// 换台机器必然不存在。不兜底的话，别人解压了 zip 也点不开 md（`is_file()` 为假就
/// 退回"打开所在文件夹"），那 zip 里带这个查看器就白带了。
pub fn bundled_preview_exe() -> Option<PathBuf> {
    let dir = exe_dir()?;
    // 两个位置都要找：
    //   - `MD-Preview.exe`          —— 便携版，就放在主程序旁边；
    //   - `tools\MD-Preview.exe`    —— 安装版，打包资源**保留目录结构**，装完在子目录里
    // 只认前者的安装版等于找不到查看器（2026-09-24 用 7z 列安装包内容时确认的）
    for rel in ["MD-Preview.exe", "tools/MD-Preview.exe"] {
        let p = dir.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// 建好数据根下需要的目录；失败返回错误文本
pub fn ensure_dirs() -> Result<(), String> {
    let root = data_root();
    std::fs::create_dir_all(root).map_err(|e| format!("创建数据目录失败 {}: {}", root.display(), e))?;
    std::fs::create_dir_all(log_dir())
        .map_err(|e| format!("创建日志目录失败 {}: {}", log_dir().display(), e))?;
    Ok(())
}

/// 剥掉 Windows verbatim（`\\?\`）前缀。
///
/// **为什么必须有**：`canonicalize` 在 Windows 上返回 `\\?\E:\...`。Rust 自己的 fs 认，
/// 但交给外部程序（explorer / MD-Preview）和展示给用户时就会出问题。
/// 保守策略：只剥盘符形式与 UNC，含 `.`/`..` 组件的不剥（语义会变），设备路径不剥。
pub fn simplify_path(p: &Path) -> PathBuf {
    if !cfg!(windows) {
        return p.to_path_buf();
    }
    let s = p.to_string_lossy();
    let stripped = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        let mut chars = rest.chars();
        match (chars.next(), chars.next()) {
            (Some(drive), Some(':')) if drive.is_ascii_alphabetic() => rest.to_string(),
            _ => return p.to_path_buf(),
        }
    } else {
        return p.to_path_buf();
    };
    let out = PathBuf::from(&stripped);
    if out
        .components()
        .any(|c| matches!(c, std::path::Component::CurDir | std::path::Component::ParentDir))
    {
        return p.to_path_buf();
    }
    out
}
