//! 打开产出：白名单 + 不代跑 + md 交给 MD-Preview。
//!
//! 这套口径是从插件搬过来的，每一条都对应一次真实事故：
//!   - **白名单**：待办里的 `files` 可能被人（或被 AI）塞进任意路径，
//!     点一下就跟着去开，等于让别人替你决定执行什么；
//!   - **可执行文件不代跑**：`.exe` / `.bat` / `.ps1` 这类默认动作是"直接执行"，
//!     命中就退化成"打开所在文件夹并选中"；
//!   - **只看 `files` 字段**：不去正文里正则捞路径 —— 正文里的绝对路径绝大多数不是产出，
//!     是参考素材、粘贴的命令、报错信息。捞出来摆在"产出"下面是误导。
//!
//! 返回值给前端当提示文案用：`ok` / `blocked` / `outside` / `missing` / `error:...`

use std::path::{Path, PathBuf};
use std::process::Command;
// Windows 专属：能把参数**原样**拼进命令行，不做引号转义。
// `explorer /select,` 这种"参数内部还有自己的分隔规则"的外部程序必须用它。
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::config::{self, AppConfig};
use crate::paths::{bundled_preview_exe, simplify_path};

/// 不代跑的扩展名：默认动作是直接执行，点一下就跑不该由程序替用户决定
const NO_LAUNCH_EXTS: &[&str] = &[
    ".exe", ".bat", ".cmd", ".com", ".msi", ".msp", ".scr", ".ps1", ".psm1", ".vbs", ".vbe",
    ".js", ".jse", ".wsf", ".wsh", ".hta", ".lnk", ".jar", ".appref-ms", ".dll", ".sys", ".drv",
];

/// 查看器吃得下的扩展名（它只认 .md / .txt，别的进去是空白页）
const PREVIEW_EXTS: &[&str] = &[".md", ".txt"];

pub fn open_path(raw: &str, mode: &str) -> String {
    let cfg = config::load();
    let p = resolve_path(raw, &cfg);

    if p.is_dir() {
        return open_folder(&p, None);
    }
    if !p.exists() {
        return "missing".to_string();
    }
    if !in_whitelist(&p, &cfg) {
        log::warn!("[打开产出] 路径不在白名单内，已忽略: {}", p.display());
        return "outside".to_string();
    }
    if mode == "folder" {
        return open_folder(&p, Some(&p));
    }
    // 可执行文件：不代跑，改成打开所在文件夹并选中
    if is_launch_blocked(&p) {
        open_folder(&p, Some(&p));
        return "blocked".to_string();
    }
    if cfg.md_open_mode == "preview" && is_previewable(&p) {
        // 找查看器：先认配置里那个，再认**主程序旁边**那个。
        // 后者是给发布版兜底的 —— 配置里的默认值是作者本机的工具路径，
        // 换台机器就不存在了，不兜底的话便携版里带的查看器永远用不上
        let configured = PathBuf::from(&cfg.preview_exe);
        let exe = if configured.is_file() {
            Some(configured)
        } else {
            bundled_preview_exe()
        };
        if let Some(exe) = exe {
            match Command::new(&exe).arg(p.to_string_lossy().to_string()).spawn() {
                Ok(_) => return "ok".to_string(),
                Err(e) => log::warn!("[打开产出] 启动查看器失败 {}: {}", exe.display(), e),
            }
        }
        // 哪儿都找不到：退回"打开所在文件夹"，别什么反应都没有
        open_folder(&p, None);
        return "ok".to_string();
    }

    match opener::open(&p) {
        Ok(()) => "ok".to_string(),
        Err(e) => {
            log::warn!("[打开产出] 打开失败 {}: {}", p.display(), e);
            format!("error:{}", e)
        }
    }
}

/// 相对路径（以库根为基准）还原成绝对路径；`simplify_path` 剥掉 `\\?\`
fn resolve_path(raw: &str, cfg: &AppConfig) -> PathBuf {
    let p = PathBuf::from(raw);
    let p = if p.is_absolute() {
        p
    } else {
        PathBuf::from(&cfg.library_root).join(p)
    };
    simplify_path(&p)
}

fn in_whitelist(p: &Path, cfg: &AppConfig) -> bool {
    // 两边都 canonicalize：白名单写的是一个目录，
    // 实际要开的可能是它下面的某个文件，还可能经过盘符大小写差异
    let target = canonical(p);
    for root in &cfg.open_roots {
        if target.starts_with(&canonical(Path::new(root))) {
            return true;
        }
    }
    false
}

/// 整成 explorer 认得出来的路径：反斜杠分隔、不带 `\\?\` 前缀的朴素路径。
///
/// **这是真踩过一次的坑**：待办里的 `files` 是 AI 写的，写成 `E:/某某/产出.jpg` 很常见。
/// Windows 自己的文件 API 认正斜杠，所以上面 `exists()`、白名单检查一路顺风，
/// **偏偏 explorer 不认** —— 它拿到 `/select,"E:/某某/产出.jpg"` 当无效参数处理，
/// 表现就是"资源管理器确实弹出来了，但停在它自己的默认位置，压根没去那个文件夹"。
/// 同一条路径把 `/` 换成 `\` 就正常了（实测过，别凭想象改回去）。
///
/// `canonicalize` 顺手把盘符大小写、`.`/`..` 都理成真身，再用 `simplify_path` 剥掉 `\\?\`
/// —— 这两样交给 explorer 同样不保险，一次做干净。
fn explorer_path(p: &Path) -> String {
    simplify_path(&canonical(p)).to_string_lossy().replace('/', "\\")
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn ext_lower(p: &Path) -> String {
    p.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn is_launch_blocked(p: &Path) -> bool {
    let ext = ext_lower(p);
    !ext.is_empty() && NO_LAUNCH_EXTS.contains(&ext.as_str())
}

fn is_previewable(p: &Path) -> bool {
    let ext = ext_lower(p);
    !ext.is_empty() && PREVIEW_EXTS.contains(&ext.as_str())
}

/// 打开文件夹；传了 `select` 就用 `explorer /select,` 顺带选中那个文件
///
/// **路径必须先过 `explorer_path`** —— explorer 是这套操作里唯一"有自己的脾气"的外部程序，
/// 它不认正斜杠，也不一定认 `\\?\` 前缀。详见那个函数的说明。
///
/// **参数必须用 `raw_arg` 传**，不能用 `arg`：`/select,` 后面的东西是 explorer 自己的一套
/// 分隔规则，`arg` 会把整串看成一个"普通参数"再加引号转义，explorer 收到的是被改造过的东西。
/// 凡是"参数内部还有自己分隔规则"的外部程序（explorer /select、cmd /c 之类），
/// 一律 `raw_arg` 原样交给它，别让 Rust 插手。
fn open_folder(dir: &Path, select: Option<&Path>) -> String {
    let mut cmd = Command::new("explorer");
    let shown = match select {
        Some(p) => format!("/select,\"{}\"", explorer_path(p)),
        None => format!("\"{}\"", explorer_path(dir)),
    };
    log::info!("[打开产出] explorer {}", shown);
    #[cfg(windows)]
    {
        cmd.raw_arg(&shown);
    }
    #[cfg(not(windows))]
    {
        cmd.arg(&shown);
    }
    match cmd.spawn() {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            log::warn!("[打开产出] 启动 explorer 失败: {}", e);
            format!("error:{}", e)
        }
    }
}
