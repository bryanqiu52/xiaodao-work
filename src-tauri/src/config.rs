//! 应用配置：落 `数据根\config.json`。
//!
//! 两件事必须照做：
//!   1. **写锁**：所有「读-改-写」都要持 `lock()`，否则并发下旧快照互相覆盖，
//!      典型事故是刚改完的设置被另一个命令用启动时的旧值整体覆写，表现为"设置自己变回去了"。
//!   2. **原子写**：配置写坏不至于丢数据，但会让应用起不来（解析失败回退默认值，用户一脸懵）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use crate::fs;
use crate::paths;

static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// 配置写锁（所有读-改-写的调用点都必须持有）
pub fn lock() -> MutexGuard<'static, ()> {
    CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- 默认值 ----

// 这里原来写着 `E:\Digital World\...`（作者本机的笔记库）。**已经删掉**：
// 默认值里出现某个人的绝对路径，就意味着换台机器它指着一个不存在的文件。
// 发布版尤其致命 —— 别人一打开待办面板就是空的，还以为是软件坏了。
// 现在的默认值一律落在**自己的数据根**下（见下面几个 default_*）。

pub const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+Space";

/// 默认暗色：界面配色就是照原插件那套暗色调的，亮色是备选
fn default_theme_mode() -> String {
    "dark".to_string()
}
/// 卡片/面板的不透明度：1 = 不透明（默认），调小才透
fn default_glass_opacity() -> f64 {
    1.0
}
/// 窗口圆角：16px，对齐 iOS 的"大卡片"档（0 = 直角）
fn default_window_radius() -> u32 {
    16
}
/// 壁纸蒙版：默认压掉三成，保证卡片上的字读得清
fn default_wallpaper_veil() -> f64 {
    0.3
}
/// 到期提醒默认提前一天：当天才提醒往往已经来不及准备了
fn default_remind_advance() -> u32 {
    1
}
// 番茄钟默认时长：25/5/15 是市面主流（Pomodoro 原始配方），4 轮后长休
fn default_focus_work() -> u32 {
    25
}
fn default_focus_short() -> u32 {
    5
}
fn default_focus_long() -> u32 {
    15
}
fn default_focus_rounds() -> u32 {
    4
}
fn default_shortcut() -> String {
    DEFAULT_SHORTCUT.to_string()
}
fn default_close_behavior() -> String {
    "hide".to_string()
}
/// 默认待办文件：**自己数据根下的 `待办.json`**（便携版就是 `exe\data\待办.json`）。
///
/// 作者本机那份已经在 `config.json` 里存着，不受这个默认值影响。
fn default_todo_file() -> String {
    crate::paths::data_root()
        .join("待办.json")
        .to_string_lossy()
        .into_owned()
}
/// 默认库根 = 数据根。它既是 `files` 里相对路径的基准，也是打开产出的白名单第一项
fn default_library_root() -> String {
    crate::paths::data_root().to_string_lossy().into_owned()
}
fn default_view() -> String {
    "user".to_string()
}
fn default_md_mode() -> String {
    "preview".to_string()
}
/// md 查看器（沿用插件里的路径；找不到时自动退回"打开所在文件夹"）
///
/// 便携版优先：**发布包把 `MD-Preview.exe` 放在主程序旁边**，这种要先认它。
/// 写死某个绝对路径只对作者本机成立，别人装了就是这个文件不存在。
fn default_preview_exe() -> String {
    if let Some(p) = crate::paths::bundled_preview_exe() {
        return p.to_string_lossy().into_owned();
    }
    r"C:\Users\123\.agents\tools\md-preview\MD-Preview.exe".to_string()
}
fn default_open_roots() -> Vec<String> {
    // 白名单默认只给**自己的地盘**（数据根）。原来还写死了作者本机的开发目录 ——
    // 那条对别人没意义，留在默认值里只会让"为什么这个路径在白名单里"变得莫名其妙
    vec![crate::paths::data_root().to_string_lossy().into_owned()]
}
fn default_true() -> bool {
    true
}
fn default_keep() -> u32 {
    14
}
/// 界面字号百分比（100 = 原始大小；只缩放字号与行高，间距不动）
fn default_ui_scale() -> u32 {
    100
}
/// 字号缩放的安全区间：再小就看不清，再大窄窗口里会挤成一团
pub const UI_SCALE_MIN: u32 = 85;
pub const UI_SCALE_MAX: u32 = 200;
/// 首次启动的窗口尺寸（逻辑像素）：窄而高，像一根竖着的清单条
pub fn default_width() -> f64 {
    450.0
}
pub fn default_height() -> f64 {
    980.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    #[serde(default = "default_width")]
    pub width: f64,
    #[serde(default = "default_height")]
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            x: None,
            y: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    // ---- 外观 ----
    /// light / dark / system
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    /// 浅色主题下的强调色 hex；空串 = 用浅色内置那支（蓝）
    #[serde(default)]
    pub accent_light: String,
    /// 深色主题下的强调色 hex；空串 = 用暗色内置那支（荧光绿）
    #[serde(default)]
    pub accent_dark: String,
    /// 旧字段：深浅共用一支强调色（1.0.2 起不再使用，只在启动时迁移给 accent_light）
    #[serde(default)]
    pub accent_color: String,
    /// 界面字号百分比（85–140，默认 100）
    #[serde(default = "default_ui_scale")]
    pub ui_scale: u32,
    /// 卡片/面板背景不透明度（0.4–1.0，默认 1）。乘进卡面底色的 alpha，不是整窗口透明
    #[serde(default = "default_glass_opacity")]
    pub glass_opacity: f64,
    /// 窗口四角圆角（逻辑像素，0–24，默认 16）
    #[serde(default = "default_window_radius")]
    pub window_radius: u32,
    /// 卡片模糊度（px，0 = 不糊）：卡片背后的东西被糊多少。
    /// 默认 0 —— 每张卡片开一层模糊是实打实的合成开销，要的人自己开
    #[serde(default)]
    pub card_blur: u32,
    /// 壁纸图片路径（空 = 不启用）。选图后会被拷进数据根，原图挪走也不失效
    #[serde(default)]
    pub wallpaper_path: String,
    /// 壁纸蒙版浓度（0–0.85）
    #[serde(default = "default_wallpaper_veil")]
    pub wallpaper_veil: f64,
    /// 壁纸整体模糊（沉浸模式下自动停用）
    #[serde(default = "default_true")]
    pub wallpaper_blur: bool,
    /// 沉浸模式：卡片变真毛玻璃，透出壁纸的模糊轮廓
    #[serde(default)]
    pub wallpaper_immersive: bool,
    /// 壁纸是否覆盖到顶部标题栏。关掉则标题栏保留自己的底色
    #[serde(default = "default_true")]
    pub wallpaper_header: bool,

    // ---- 常规 ----
    #[serde(default = "default_shortcut")]
    pub global_shortcut: String,
    /// 关窗行为：hide（藏到托盘）/ quit（退出）
    #[serde(default = "default_close_behavior")]
    pub close_behavior: String,
    #[serde(default)]
    pub always_on_top: bool,
    /// 贴边隐藏：拖到屏幕左右边缘停住后自动收起来，鼠标划到边缘再滑出
    #[serde(default = "default_true")]
    pub edge_hide: bool,
    #[serde(default)]
    pub window: WindowState,

    // ---- 待办 ----
    /// 待办文件绝对路径（与 AI 共用同一份）
    #[serde(default = "default_todo_file")]
    pub todo_file: String,
    /// 上次「复制变更说明」时用过的待办路径（为了能说出"从哪变到哪"）
    #[serde(default)]
    pub notified_todo_file: String,
    /// 同上，数据目录那一份
    #[serde(default)]
    pub notified_data_root: String,
    /// 库根：打开产出的根目录
    #[serde(default = "default_library_root")]
    pub library_root: String,
    /// 默认视图：user（我的）/ agent（小刀的）
    #[serde(default = "default_view")]
    pub default_view: String,
    /// 显示已删除条目（只影响列表展示，不删记录）
    #[serde(default)]
    pub show_deleted: bool,

    // ---- 打开产出 ----
    /// md 打开方式：preview（MD-Preview.exe）/ system（系统默认）
    #[serde(default = "default_md_mode")]
    pub md_open_mode: String,
    #[serde(default = "default_preview_exe")]
    pub preview_exe: String,
    /// 允许打开的目录白名单（防误开任意路径）
    #[serde(default = "default_open_roots")]
    pub open_roots: Vec<String>,

    // ---- 数据 ----
    #[serde(default = "default_true")]
    pub backup_enabled: bool,
    #[serde(default = "default_keep")]
    pub backup_keep: u32,

    // ---- 到期提醒 ----
    /// 到期提醒总开关
    #[serde(default = "default_true")]
    pub remind_enabled: bool,
    /// 提前几天提醒（0 = 当天才提醒）
    #[serde(default = "default_remind_advance")]
    pub remind_advance_days: u32,
    /// 最近一次提醒的日期（YYYY-MM-DD）。跟今天对不上就说明跨天了，当天记录作废
    #[serde(default)]
    pub remind_day: String,
    /// 当天已提醒过的条目 id。**只记当天、不留历史** —— 留历史会无限增长
    #[serde(default)]
    pub remind_done: Vec<String>,
    /// 被「稍后再说」推迟的条目 id
    #[serde(default)]
    pub remind_snoozed: Vec<String>,
    /// 推迟到什么时候（毫秒时间戳；0 = 没有推迟）
    #[serde(default)]
    pub remind_snooze_until: i64,

    // ---- 番茄闹钟 ----
    /// 专注时长（分钟）
    #[serde(default = "default_focus_work")]
    pub focus_work_min: u32,
    /// 短休时长（分钟）
    #[serde(default = "default_focus_short")]
    pub focus_short_min: u32,
    /// 长休时长（分钟）
    #[serde(default = "default_focus_long")]
    pub focus_long_min: u32,
    /// 长休前的专注轮数
    #[serde(default = "default_focus_rounds")]
    pub focus_rounds: u32,
    /// 阶段结束后自动开始下一阶段
    #[serde(default)]
    pub focus_auto_continue: bool,
    /// 到点提示音（系统通知始终发，这里只管响不响）
    #[serde(default = "default_true")]
    pub focus_sound: bool,
    /// 今日完成番茄数的日期（YYYY-MM-DD）；跟今天对不上就清零
    #[serde(default)]
    pub focus_done_day: String,
    /// 今日完成的番茄数（只记当天，跨天重置）
    #[serde(default)]
    pub focus_done_count: u32,

    // ---- 领域分类（可编辑）----
    /// 领域分类的 key 列表；**空 = 用前端内置那 6 类**。
    /// 这里只是存盘（判定在前端），所以不加任何校验 —— 用户填什么都能存
    #[serde(default)]
    pub domains: Vec<String>,
    /// 分类的中文名
    #[serde(default)]
    pub domain_labels: HashMap<String, String>,

    /// 首次启动向导看过了没
    #[serde(default)]
    pub welcome_done: bool,

    /// 上次查更新的本地日期（`YYYY-MM-DD`）；空串 = 从没查过。
    /// 「一天最多查一次」的节流靠它
    #[serde(default)]
    pub update_checked_at: String,

    /// 用户说了"这一版先不更新"的版本号；出现更新的版本时照样会弹
    #[serde(default)]
    pub update_skipped_version: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme_mode: default_theme_mode(),
            accent_light: String::new(),
            accent_dark: String::new(),
            accent_color: String::new(),
            ui_scale: default_ui_scale(),
            glass_opacity: default_glass_opacity(),
            window_radius: default_window_radius(),
            card_blur: 0,
            wallpaper_path: String::new(),
            wallpaper_veil: default_wallpaper_veil(),
            wallpaper_blur: true,
            wallpaper_immersive: false,
            wallpaper_header: true,
            global_shortcut: default_shortcut(),
            close_behavior: default_close_behavior(),
            always_on_top: false,
            edge_hide: default_true(),
            window: WindowState::default(),
            todo_file: default_todo_file(),
            notified_todo_file: String::new(),
            notified_data_root: String::new(),
            library_root: default_library_root(),
            default_view: default_view(),
            show_deleted: false,
            md_open_mode: default_md_mode(),
            preview_exe: default_preview_exe(),
            open_roots: default_open_roots(),
            backup_enabled: default_true(),
            backup_keep: default_keep(),
            remind_enabled: true,
            remind_advance_days: default_remind_advance(),
            remind_day: String::new(),
            remind_done: Vec::new(),
            remind_snoozed: Vec::new(),
            remind_snooze_until: 0,
            focus_work_min: default_focus_work(),
            focus_short_min: default_focus_short(),
            focus_long_min: default_focus_long(),
            focus_rounds: default_focus_rounds(),
            focus_auto_continue: false,
            focus_sound: true,
            focus_done_day: String::new(),
            focus_done_count: 0,
            domains: Vec::new(),
            domain_labels: HashMap::new(),
            welcome_done: false,
            update_checked_at: String::new(),
            update_skipped_version: String::new(),
        }
    }
}

/// 读配置；文件不存在或解析失败都回退默认（不让一个坏配置把应用卡死）
pub fn load() -> AppConfig {
    let path = paths::config_file();
    match fs::read_text(&path) {
        Ok(text) => match serde_json::from_str::<AppConfig>(&text) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[配置] 解析失败，已回退默认值: {} ({})", path.display(), e);
                AppConfig::default()
            }
        },
        Err(_) => AppConfig::default(),
    }
}

/// 写配置（原子写）
pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let path = paths::config_file();
    let text = serde_json::to_string_pretty(cfg).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::atomic_write(&path, text.as_bytes())
}

/// 只改一个字段的便捷入口：读 → 改 → 写，全程持锁
pub fn update<F: FnOnce(&mut AppConfig)>(f: F) -> Result<AppConfig, String> {
    let _guard = lock();
    let mut cfg = load();
    f(&mut cfg);
    save(&cfg)?;
    Ok(cfg)
}
