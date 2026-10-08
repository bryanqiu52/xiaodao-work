//! xiaodao_work 的 Rust 侧：只做「文件壳 + 系统能力」，不解析待办业务语义。
//!
//! 业务规则全部在前端 `src/core/`，
//! 前端是唯一的业务真相持有者；这里只负责字节级读写、监听、备份、托盘、快捷键、打开产出。

mod autostart;
mod backup;
mod commands;
mod desktop;
mod edge;
mod config;
mod focus_log;
mod fs;
mod github;
mod money;
mod open;
mod paths;
mod reminder;
mod shortcut;
mod todo_io;
mod tray;
mod watcher;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{LogicalPosition, LogicalSize, Manager, WebviewWindow};
// tauri-plugin-log 2.9 的写法：`Target` 是结构体，具体去向放在 `TargetKind` 里
// （早期版本是直接 `LogTarget::Stdout` 这样的枚举，照老写法编不过）
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy};

/// 窗口尺寸位置的保存节流：拖窗口会连续触发上百次事件，没必要每次都写盘
static LAST_SAVE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                // 默认打的是 UTC，比本地早 8 小时 —— 排查时会以为日志"停在几小时前"，
                // 白白绕远路。直接用本地时间。
                .timezone_strategy(TimezoneStrategy::UseLocal)
                // 默认连 notify 的 TRACE 都写：待办文件每被改动一次就刷好几行原始事件，
                // 真正要看的「写冲突 / 备份失败」全被淹了。这里只留 Info 及以上。
                .level(log::LevelFilter::Info)
                .level_for("notify", log::LevelFilter::Warn)
                // `Asset 'favicon.ico' not found` 之类的 DEBUG 噪音同理
                .level_for("tauri::manager", log::LevelFilter::Warn)
                .targets([
                    Target::new(TargetKind::Folder {
                        path: paths::log_dir(),
                        file_name: None,
                    }),
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            // 开机自启：注册时带上 `--autostart`，启动时靠它区分"用户点开的"和"开机拉起来的"
            // （后者不弹窗、直接托盘待命）
            tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--autostart"]),
            ),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    shortcut::on_shortcut(app, shortcut, event.state());
                })
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二次启动：把已经跑着的那个叫出来，不再开一个
            tray::show_window(app);
        }))
        // 自动更新：查最新版 / 下载 / **校验签名** / 跑安装包，全由这个插件做。
        // 签名校验的根是 `tauri.conf.json` 里那个公钥 —— 私钥在 CI 的 Secret 里，
        // 所以别人就算换了 release 上的包，也签不出能通过校验的版本。
        .plugin(tauri_plugin_updater::Builder::new().build())
        // 装完之后把应用重新拉起来（updater 自己不管"重启"这件事）
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::todo_read,
            commands::todo_write,
            commands::config_get,
            commands::config_set,
            commands::backup_now,
            commands::backup_list,
            commands::backup_entries,
            commands::backup_restore,
            commands::notify,
            commands::is_portable,
            commands::data_root_path,
            commands::data_root_custom,
            commands::data_root_set,
            commands::data_root_reset,
            commands::data_reset,
            commands::open_path,
            commands::pick_directory,
            commands::pick_file,
            commands::set_shortcut,
            commands::show_window,
            commands::edge_reveal,
            commands::edge_hide,
            commands::shortcut_state,
            commands::win_minimize,
            commands::win_toggle_maximize,
            commands::win_close,
            commands::win_set_always_on_top,
            commands::win_start_drag,
            commands::frontend_log,
            commands::env_probe,
            commands::import_wallpaper,
            commands::remove_wallpaper,
            commands::builtin_wallpapers,
            commands::open_url,
            commands::github_trending,
            commands::github_open,
            commands::money_read,
            commands::money_write,
            commands::focus_log_read,
            commands::focus_log_append,
            // 自启这两条直接在 autostart 模块上（命令标在那里，不必再转发一层）
            autostart::autostart_info,
            autostart::autostart_set,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // tauri.conf.json 里窗口是 `visible: false`，照常启动时才 show。
            // 开机自启拉起来时会带上 `--autostart`（见上面注册插件那处），据此跳过 show ——
            // 开机就弹一个窗口出来很打扰，那句"开机后自动在托盘里待命"说的就是这个。
            let launched_by_autostart = std::env::args().any(|a| a == "--autostart");
            paths::ensure_dirs()?;

            // 一次性迁移：老配置里"待办文件另指他处"的，把那份复制进数据根。
            // **必须在下面所有读待办的动作之前** —— 不然备份、监听、界面读到的都是空文件。
            // 旧位置那份**不动**（观察两三天、确认 AI 都跟过来了再删）。
            if let Some(msg) = config::migrate_todo_file() {
                log::info!("[数据] {}", msg);
            }

            // 壁纸是本地图片，WebView 得走 asset 协议才读得到。
            // 静态 scope 只能写死路径，而数据根是运行时才定的（便携模式下还会变），
            // 所以在这里按实际路径补一次授权 —— 少了它壁纸就是一块空白，还不报错。
            let _ = app
                .asset_protocol_scope()
                .allow_directory(&paths::wallpaper_dir(), true);

            // 内置壁纸在资源目录（安装目录）下，不在数据根里 —— 不补这一次授权，
            // 设置里那些卡片缩略图就是一片白块，而且**不报错**，属于最难查的那类问题
            if let Some(dir) = paths::builtin_wallpaper_dir(app.handle()) {
                let _ = app.asset_protocol_scope().allow_directory(&dir, true);
            }

            let cfg = config::load();
            edge::set_enabled(cfg.edge_hide);

            // 贴边隐藏的守护线程：轮询判断"窗口是不是拖到边上停住了"。
            // 刻意不挂在 Moved 事件上 —— 拖动时事件连发，没法判断这一下算不算停住。
            let poll_handle = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(edge::POLL_INTERVAL);
                // peek 判定放在 check_settle 之前：屏幕上的那条缝里藏着鼠标时，
                // 优先把它叫出来，而不是先想着再收回去
                edge::poll_peek(&poll_handle);
                edge::check_settle(&poll_handle);
            });

            // 顺序有讲究：先按盘上原文备份，之后前端加载时可能就地修正完成态并写回，
            // 备份必须落在修正之前，否则回滚凭据就不是"修改前"的样子。
            if cfg.backup_enabled {
                if let Err(e) = backup::backup_daily(&paths::todo_file(), cfg.backup_keep) {
                    log::warn!("[备份] 每日备份失败: {}", e);
                }
            }

            watcher::start(&handle, paths::todo_file());
            // 记账文件也监听：AI 通过 skill 记一笔之后，界面要能自己看到（不然等于白写）
            watcher::start_money(&handle);
            // 到期提醒的节拍器：每半小时喊一声，该提醒谁由前端判定（见 reminder.rs 的注释）
            reminder::start(&handle);
            // 番茄钟的到点兜底拍子：窗口藏进托盘后 WebView 定时器被节流，靠这条保证到点通知不迟到
            reminder::start_focus_tick(&handle);
            tray::setup(&handle)?;

            if let Err(e) = shortcut::register(&handle, &cfg.global_shortcut) {
                log::warn!(
                    "[快捷键] 注册失败（{}），不影响启动，可在设置里换一个",
                    if e == "conflict" { "与别的程序冲突" } else { "格式无效" }
                );
            }

            if let Some(win) = app.get_webview_window("main") {
                let (size, pos) = restore_window(&win, &cfg);
                // 虚拟桌面这套接口在本机能不能用，启动就留一行（真出问题时它是第一个抓手）
                desktop::probe(&handle);
                // 只留托盘，不要任务栏图标（Quicker 那类常驻工具的用法）。
                // **副作用得记着**：靠的是"工具窗口"属性，Alt+Tab 里也会跟着没有 ——
                // 唤回窗口只能靠托盘左键 / 全局快捷键。
                desktop::hide_from_taskbar(&handle);
                // 启动这一阵子窗口样式会被系统 / WebView 反复重套（尺寸那件事已经
                // 领教过一次），隔几拍再确认一次，别让图标又挂回去。
                let tb_handle = handle.clone();
                std::thread::spawn(move || {
                    for step in [300u64, 700, 1200] {
                        std::thread::sleep(Duration::from_millis(step));
                        desktop::hide_from_taskbar(&tb_handle);
                    }
                });
                bind_window_events(&handle, &win);
                if launched_by_autostart {
                    // 开机拉起来的：尺寸位置照样恢复，但不弹出来。托盘图标已经在了，
                    // 用户想用时点托盘或按快捷键都能叫出来。
                    log::info!("[自启] 本次由开机自启拉起，窗口留在托盘");
                } else {
                    let _ = win.show();
                }
                // 尺寸重套在两种情况下都要做：托盘唤起时窗口也该是用户上次那个大小
                reschedule_restore(&handle, size, pos);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 xiaodao_work 失败");
}

/// 窗口尺寸的下限（比这再小就挤成一团了）
const MIN_W: f64 = 360.0;
const MIN_H: f64 = 480.0;

/// 启动时应有的窗口尺寸。
///
/// 两个讲究：
///   1. 配置里没记过（或数值不合理）就用默认 —— 之前踩过坑：默认宽高与配置里
///      夹过下限的值互相写来写去，窗口就被永久钉死在最小尺寸上。
///   2. 高度要按屏幕再夹一次 —— 系统缩放会把 980 逻辑像素放大到 1300+ 物理像素，
///      屏小的时候就有一半窗口落在屏幕外面。
fn initial_size(
    win: &WebviewWindow,
    saved_w: &f64,
    saved_h: &f64,
    scale: f64,
) -> (f64, f64) {
    // 判定用「>」而不是「>=」：正好等于下限的值是我们自己夹出来的（从来没成功记过
    // 真实尺寸），照它恢复会把窗口永久钉死在最小尺寸上。
    //
    // 另一头也要拦：记录值比屏幕还大（上次是最大化时记下来的 2560 这种），
    // 照它恢复窗口一开机就铺满屏幕 —— 用户要的默认尺寸就白设了。
    let (sw, sh) = screen_size(win, scale);
    let width = if *saved_w > MIN_W && *saved_w < sw * 0.95 {
        *saved_w
    } else {
        config::default_width()
    };
    let height = if *saved_h > MIN_H && *saved_h < sh * 0.95 {
        *saved_h
    } else {
        config::default_height()
    };
    // 高度不超过屏幕可用高度的 92%（给任务栏和视觉留白）
    let height = if sh > 0.0 { height.min(sh * 0.92) } else { height };
    (width.max(MIN_W), height.max(MIN_H))
}

/// 屏幕可用尺寸（逻辑像素）。读不到就返回一个很大值 —— 那意味着"不限制"
fn screen_size(win: &WebviewWindow, scale: f64) -> (f64, f64) {
    match win.available_monitors() {
        Ok(ms) => ms
            .iter()
            .map(|m| {
                (
                    m.size().width as f64 / scale,
                    m.size().height as f64 / scale,
                )
            })
            .fold((0.0, 0.0), |acc, v| (acc.0.max(v.0), acc.1.max(v.1))),
        Err(_) => (f64::MAX, f64::MAX),
    }
}

/// 恢复上次的窗口尺寸位置，并把「最终决定用的尺寸和位置」回填给调用方。
fn restore_window(
    win: &WebviewWindow,
    cfg: &config::AppConfig,
) -> ((f64, f64), Option<(f64, f64)>) {
    // WebView2 会替窗口记住"上次是最大化"并在下次启动时自己恢复，
    // 那样下面设的尺寸就不作数了 —— 先把它按回普通状态。
    if win.is_maximized().unwrap_or(false) {
        let _ = win.unmaximize();
    }
    let w = &cfg.window;
    let scale = win.scale_factor().unwrap_or(1.0);
    let size = initial_size(win, &w.width, &w.height, scale);
    let _ = win.set_size(LogicalSize {
        width: size.0,
        height: size.1,
    });
    // 启动尺寸对不上时不猜：把「要的」和「实际落到的」一起记下来。
    // 系统缩放会把逻辑尺寸放大成另一个物理尺寸，日志留痕看一眼就知道差在哪。
    let actual = win
        .inner_size()
        .map(|s| format!("{}x{}", s.width, s.height))
        .unwrap_or_else(|_| "读不到".to_string());
    log::info!(
        "[窗口] 启动尺寸 目标 {}x{} 实际 {} scale={:.2}",
        size.0,
        size.1,
        actual,
        scale
    );
    // 尺寸被夹住时得知道夹在什么上：把所有显示器的原始尺寸都打出来。
    // 分不清"物理像素/逻辑像素"是这类问题的头号来源，有原始数字就不用猜。
    if let Ok(ms) = win.available_monitors() {
        for m in ms {
            log::info!(
                "[窗口] 显示器 {:?} 原始 {}x{} 自身scale={:.2}",
                m.name().map(|s| s.to_string()).unwrap_or_else(|| "?".into()),
                m.size().width,
                m.size().height,
                m.scale_factor()
            );
        }
    }

    // 位置不能直接照抄配置：里面可能是上次隐藏窗口时被写进去的脏坐标，
    // 照抄会让窗口落到屏幕外，看着就像"打不开"。
    let usable = match (w.x, w.y) {
        (Some(x), Some(y)) if position_on_screen(win, x * scale, y * scale) => Some((x, y)),
        _ => None,
    };
    let applied = match usable {
        Some((x, y)) => {
            let _ = win.set_position(LogicalPosition { x, y });
            Some((x, y))
        }
        None => {
            let _ = win.center();
            // 顺便把脏坐标从配置里抹掉，免得每次启动都白算一遍
            let _ = config::update(|c| {
                c.window.x = None;
                c.window.y = None;
            });
            None
        }
    };
    let _ = win.set_always_on_top(cfg.always_on_top);
    (size, applied)
}

/// 过一会儿再把尺寸位置**重新套一遍**。
///
/// **为什么必须**：WebView2 会记住窗口上次的尺寸（包括最大化时的巨大尺寸），
/// 并在 webview 初始化时自己恢复 —— 这一步比 setup 晚，会把我们在
/// `restore_window()` 里设好的尺寸整个盖掉，表现为"默认尺寸根本不生效"。
fn reschedule_restore(handle: &tauri::AppHandle, size: (f64, f64), pos: Option<(f64, f64)>) {
    let h = handle.clone();
    std::thread::spawn(move || {
        // **分几次重试**：系统把上次窗口状态盖回来的时机不定（有时还带着最大化），
        // 只补一次压不住。一旦量到宽度对上了就收手，别没完没了地跟窗口较劲。
        let mut waited = 0u64;
        for step in [300u64, 400, 500, 700, 1000] {
            std::thread::sleep(Duration::from_millis(step));
            waited += step;
            let Some(win) = h.get_webview_window("main") else {
                return;
            };
            if win.is_maximized().unwrap_or(false) {
                let _ = win.unmaximize();
            }
            let _ = win.set_size(LogicalSize {
                width: size.0,
                height: size.1,
            });
            match pos {
                Some((x, y)) => {
                    let _ = win.set_position(LogicalPosition { x, y });
                }
                None => {
                    let _ = win.center();
                }
            }
            // 对比时务必把逻辑尺寸换算成物理：`inner_size()` 报物理像素，
            // `size` 是逻辑像素，缩放 125% 时两者差着 1/5，直接比会永远"没稳定"。
            let scale = win.scale_factor().unwrap_or(1.0);
            let want_w = size.0 * scale;
            let ok = win
                .inner_size()
                .map(|s| (s.width as f64 - want_w).abs() < 24.0)
                .unwrap_or(false);
            if ok {
                log::info!("[窗口] 尺寸已稳定在 {}x{}（{}ms 后）", size.0, size.1, waited);
                return;
            }
        }
        log::warn!(
            "[窗口] 尺寸没能稳定到 {}x{}，最后一次实测见上一条日志",
            size.0,
            size.1
        );
    });
}

/// 窗口左上角是否还落在某一块屏幕上（物理像素坐标）。
///
/// **为什么必须有**：Windows 会给隐藏的窗口赋 (-32000, -32000) 这种"屏幕外"坐标，
/// 那个值被当成真实位置存下来，下次启动窗口就再也找不回来了。
fn position_on_screen(win: &WebviewWindow, x: f64, y: f64) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    match win.available_monitors() {
        Ok(monitors) => monitors.iter().any(|m| {
            let left = m.position().x as f64;
            let top = m.position().y as f64;
            let size = m.size();
            x >= left - 8.0
                && x <= left + size.width as f64 - 8.0
                && y >= top - 8.0
                && y <= top + size.height as f64 - 8.0
        }),
        Err(_) => false,
    }
}

fn bind_window_events(app: &tauri::AppHandle, win: &WebviewWindow) {
    let handle = app.clone();
    win.on_window_event(move |event| match event {
        // 关窗口 ≠ 退出：默认藏到托盘（设置里可改成直接退出）
        tauri::WindowEvent::CloseRequested { api, .. } => {
            if config::load().close_behavior != "quit" {
                api.prevent_close();
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
        }
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
            edge::mark_moved();
            if let Some(w) = handle.get_webview_window("main") {
                save_window_state(&w);
            }
        }
        // 点任务栏上的图标（或按 Alt+Tab）把窗口叫回来时，它多半还站在屏幕外面
        // 只露着那 5px —— 看着就像"点了没反应"。这里补一次滑出：拿到焦点先滑回来。
        tauri::WindowEvent::Focused(true) => {
            let hidden = edge::is_hidden();
            log::info!("[窗口] 拿到焦点（贴边收起着={}）", hidden);
            if hidden {
                // 从别处把焦点拿回来的：鼠标多半还在别处，要等它进来一次（true）
                edge::reveal(&handle, true);
            }
        }
        _ => {}
    });
}

/// 记住窗口尺寸位置（500ms 节流）
fn save_window_state(win: &WebviewWindow) {
    // 贴边收起时窗口大半在屏幕外，这个坐标记下来下次就找不着窗口了
    if edge::is_hidden() {
        return;
    }
    // 窗口不可见时**不要**记录位置：隐藏状态下 Windows 会把它挪到 (-32000,-32000)，
    // 那是"已隐藏"的标记而不是真实位置，记下来下次就找不着窗口了。
    if !win.is_visible().unwrap_or(false) {
        return;
    }
    // 最大化时的尺寸是"整块屏幕"，记下来下次就按铺满屏幕的尺寸开 —— 不是用户要的尺寸
    if win.is_maximized().unwrap_or(false) {
        return;
    }

    let mut last = match LAST_SAVE.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let now = Instant::now();
    if let Some(t) = *last {
        if now.duration_since(t) < Duration::from_millis(500) {
            return;
        }
    }
    *last = Some(now);
    drop(last);

    let size = match win.inner_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let pos = match win.outer_position() {
        Ok(p) => p,
        Err(_) => return,
    };
    // 兜底：真读到屏幕外的坐标就宁可不存，别把下次的启动位置带沟里
    if !position_on_screen(win, pos.x as f64, pos.y as f64) {
        return;
    }
    let scale = win.scale_factor().unwrap_or(1.0);
    let _ = config::update(|cfg| {
        cfg.window.width = size.width as f64 / scale;
        cfg.window.height = size.height as f64 / scale;
        cfg.window.x = Some(pos.x as f64 / scale);
        cfg.window.y = Some(pos.y as f64 / scale);
    });
}
