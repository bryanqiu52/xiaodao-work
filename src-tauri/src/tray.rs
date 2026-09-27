//! 托盘：常驻后台，左键单击显隐主窗，右键菜单（打开 / 设置 / 退出）。
//!
//! 这个应用就该是托盘常驻的用法：想到就按快捷键叫出来，不用去任务栏找。

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let open = MenuItemBuilder::with_id("open", "打开小刀工作台")
        .build(app)
        .map_err(|e| e.to_string())?;
    let settings = MenuItemBuilder::with_id("settings", "设置")
        .build(app)
        .map_err(|e| e.to_string())?;
    let quit = MenuItemBuilder::with_id("quit", "退出")
        .build(app)
        .map_err(|e| e.to_string())?;
    let menu = MenuBuilder::new(app)
        .items(&[&open, &settings, &quit])
        .build()
        .map_err(|e| e.to_string())?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("小刀工作台")
        .menu(&menu)
        .on_menu_event(|app, event| {
            // 托盘点击没有界面反馈，出问题只能靠日志——每次点击都留个痕
            let id = event.id().as_ref().to_string();
            log::info!("[托盘] 菜单被点击: {}", id);
            match id.as_str() {
                "open" => show_window(app),
                "settings" => {
                    show_window(app);
                    match app.emit("open-settings", ()) {
                        Ok(_) => log::info!("[托盘] 已派发 open-settings 事件"),
                        Err(e) => log::warn!("[托盘] 派发 open-settings 失败: {}", e),
                    }
                }
                "quit" => app.exit(0),
                other => log::warn!("[托盘] 没认出来的菜单项: {}", other),
            }
        })
        .on_tray_icon_event(|icon, event| {
            // 左键**抬起**才切换：按下就切会和拖拽、双击打架
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(icon.app_handle());
            }
        });

    // `default_window_icon()` 借出来的是引用，托盘要的是所有权，所以 clone 一份
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn show_window(app: &AppHandle) {
    // 先跟到用户此刻所在的那个虚拟桌面：窗口要是留在别的桌面，
    // 这里 show 出来他也看不见 —— 系统不会替我们跨桌面搬家。
    crate::desktop::follow_current_desktop(app);
    // 贴边收起时窗口是"多半在屏幕外"的状态，先滑回来再显示，
    // 不然托盘点一下只在边上露出一条缝。
    if crate::edge::is_hidden() {
        // 托盘 / 快捷键唤起：鼠标还在托盘那边，要等它进来一次（true）
        crate::edge::reveal(app, true);
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn toggle_window(app: &AppHandle) {
    // 贴边收起时，点托盘的本意**一定是**"把它叫出来"，不能走"再点一下收起来"那条分支
    // —— 窗口本来就是可见的，那条分支会把它 hide 掉，看着就是"点了没用"。
    // 交给 show_window：它会先把窗口从边上滑回来。
    if crate::edge::is_hidden() {
        show_window(app);
        return;
    }
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        if visible && win.is_focused().unwrap_or(false) {
            let _ = win.hide();
        } else {
            // 同上：显示之前先确认它在当前这个虚拟桌面上
            crate::desktop::follow_current_desktop(app);
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
}
