//! 虚拟桌面跟随 + 摘掉任务栏图标（都只在 Windows 上生效）。
//!
//! ## 虚拟桌面
//!
//! **为什么需要**：Windows 的规矩是 —— 一个窗口同一时刻只属于一个虚拟桌面。
//! 窗口开在桌面 1，用户切到桌面 2 再点托盘，系统不会把它带过来，
//! 表现就是"点了没反应"；托盘图标倒是所有桌面都在（它属于系统外壳，不归桌面管）。
//!
//! **做法是"跟随"，不是"钉在所有桌面"**：后者（任务视图里那个
//! "在所有桌面显示此窗口"）只有**未文档化**的 COM 接口，虚函数表随 Windows
//! 版本挪位置，升级一次就可能失效。这里只用写进文档的 IVirtualDesktopManager 三件套：
//!   · IsWindowOnCurrentVirtualDesktop —— 我这个窗口在不在当前桌面
//!   · GetWindowDesktopId             —— 某个窗口在哪个桌面
//!   · MoveWindowToDesktop            —— 把窗口挪过去
//! 代价是"同一时刻只在一个桌面"，换来的是稳。
//!
//! **拿不到信息时一律当"已经在当前桌面"**：这一步失败也绝不能拦着显示窗口 ——
//! 窗口没跟过来（用户再点一次托盘）比"点了根本不显示"要好得多。
//!
//! ## 任务栏
//!
//! **为什么自己改窗口样式**：`WebviewWindow::set_skip_taskbar(true)` 在这套版本上
//! **调用成功却不起作用** —— 实测改完 exStyle 里 `WS_EX_APPWINDOW` 还在、
//! `WS_EX_TOOLWINDOW` 没加上，图标照样挂在任务栏。所以直接动窗口扩展样式，
//! 改完再 `SetWindowPos(… SWP_FRAMECHANGED)` 逼系统重画一次边框（少了这步改了也不生效）。
//!
//! **代价得记着**：`WS_EX_TOOLWINDOW` 的窗口不进 Alt+Tab。这是 Quicker 那类常驻
//! 工具的常规做法 —— 唤回窗口靠托盘左键 / 全局快捷键。

use tauri::{AppHandle, Manager, WebviewWindow};

/// 唤起窗口前调一次：窗口不在当前桌面就把它搬过来。
/// 返回 true 表示"确实搬了"。**false 也要照常显示窗口**。
pub fn follow_current_desktop(app: &AppHandle) -> bool {
    let Some(win) = app.get_webview_window("main") else {
        return false;
    };
    follow(&win)
}

/// 启动时自检一行：这套虚拟桌面接口在本机到底能不能用。
///
/// **为什么要有**：跨桌面这件事只能在真机上、在两个桌面之间试，代码里自证不了。
/// 留一行日志，用户报"还是叫不出来"时先看它，就能分开是"接口这台机器不给用"
/// 还是"逻辑走岔了" —— 省掉一半的猜。
pub fn probe(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    probe_win(&win);
}

/// 把窗口从任务栏（以及 Alt+Tab）里摘掉，只留托盘。
/// 返回 true 表示"这次真的改了样式"。
pub fn hide_from_taskbar(app: &AppHandle) -> bool {
    let Some(win) = app.get_webview_window("main") else {
        return false;
    };
    hide_from_taskbar_win(&win)
}

#[cfg(target_os = "windows")]
fn follow(win: &WebviewWindow) -> bool {
    let Some(hwnd) = hwnd_of(win) else {
        log::warn!("[桌面] 拿不到窗口句柄，跳过跟随");
        return false;
    };
    unsafe { follow_hwnd(hwnd) }
}

#[cfg(not(target_os = "windows"))]
fn follow(_win: &WebviewWindow) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn hide_from_taskbar_win(win: &WebviewWindow) -> bool {
    let Some(hwnd) = hwnd_of(win) else {
        log::warn!("[任务栏] 拿不到窗口句柄，跳过");
        return false;
    };
    unsafe { hide_taskbar_hwnd(hwnd) }
}

#[cfg(not(target_os = "windows"))]
fn hide_from_taskbar_win(_win: &WebviewWindow) -> bool {
    false
}

/// 窗口的原生句柄（Windows 上是 HWND）。拿不到就当"这条路走不通"，
/// 调用方照常干自己的事 —— 不是致命错误。
#[cfg(target_os = "windows")]
fn probe_win(win: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(win) else {
        log::warn!("[桌面] 自检：拿不到窗口句柄");
        return;
    };
    unsafe { probe_hwnd(hwnd) }
}

#[cfg(not(target_os = "windows"))]
fn probe_win(_win: &WebviewWindow) {}

#[cfg(target_os = "windows")]
fn hwnd_of(win: &WebviewWindow) -> Option<sys::Hwnd> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let h = win.window_handle().ok()?;
    match h.as_raw() {
        RawWindowHandle::Win32(w) => {
            let p = w.hwnd.get() as sys::Hwnd;
            if p.is_null() {
                None
            } else {
                Some(p)
            }
        }
        _ => None,
    }
}

#[cfg(target_os = "windows")]
mod sys {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub struct Guid {
        pub d1: u32,
        pub d2: u16,
        pub d3: u16,
        pub d4: [u8; 8],
    }

    pub type Hresult = i32;
    pub type Hwnd = *mut c_void;

    /// `IVirtualDesktopManager`：只继承 IUnknown，后三个是它自己的方法。
    /// **顺序照 shobjidl_core.h 里的声明，一个都不能挪** —— 挪了就是拿错函数去调。
    #[repr(C)]
    pub struct Vtable {
        pub query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> Hresult,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
        pub is_on_current: unsafe extern "system" fn(*mut c_void, Hwnd, *mut i32) -> Hresult,
        pub get_desktop_id: unsafe extern "system" fn(*mut c_void, Hwnd, *mut Guid) -> Hresult,
        pub move_to_desktop: unsafe extern "system" fn(*mut c_void, Hwnd, *const Guid) -> Hresult,
    }

    /// CLSID_VirtualDesktopManager
    pub const CLSID_VDM: Guid = Guid {
        d1: 0xAA50_9086,
        d2: 0x5CA9,
        d3: 0x4C25,
        d4: [0x8F, 0x95, 0x58, 0x9D, 0x3C, 0x07, 0xB4, 0x8A],
    };
    /// IID_IVirtualDesktopManager
    pub const IID_VDM: Guid = Guid {
        d1: 0xA5CD_92FF,
        d2: 0x29BE,
        d3: 0x454C,
        d4: [0x8D, 0x04, 0xD8, 0x28, 0x79, 0xFB, 0x3F, 0x1B],
    };

    /// CoInitializeEx 发现本线程已用别的模式初始化过时会返回这个 —— 那也算能用
    pub const RPC_E_CHANGED_MODE: Hresult = 0x8001_0106u32 as i32;
    /// CLSCTX_ALL：这个 COM 对象是系统自带的进程内对象，本机各类服务器都让它试
    pub const CLSCTX_ALL: u32 = 23;
    /// COINIT_APARTMENTTHREADED
    pub const COINIT_APARTMENTTHREADED: u32 = 2;

    pub const GWL_EXSTYLE: i32 = -20;
    /// "这是个普通应用窗口、该有任务栏按钮"
    pub const WS_EX_APPWINDOW: i32 = 0x0004_0000;
    /// "这是个工具窗口、不要任务栏也不要 Alt+Tab"
    pub const WS_EX_TOOLWINDOW: i32 = 0x0000_0080;
    pub const SWP_NOSIZE: u32 = 0x0001;
    pub const SWP_NOMOVE: u32 = 0x0002;
    pub const SWP_NOZORDER: u32 = 0x0004;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
    /// 改完窗口样式要靠它逼系统重画一次，否则改了跟没改一样
    pub const SWP_FRAMECHANGED: u32 = 0x0020;

    #[link(name = "ole32")]
    extern "system" {
        pub fn CoInitializeEx(pv: *mut c_void, coinit: u32) -> Hresult;
        pub fn CoCreateInstance(
            clsid: *const Guid,
            outer: *mut c_void,
            ctx: u32,
            iid: *const Guid,
            out: *mut *mut c_void,
        ) -> Hresult;
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetForegroundWindow() -> Hwnd;
        pub fn GetShellWindow() -> Hwnd;
        pub fn GetWindowLongW(h: Hwnd, index: i32) -> i32;
        pub fn SetWindowLongW(h: Hwnd, index: i32, value: i32) -> i32;
        pub fn SetWindowPos(
            h: Hwnd,
            after: Hwnd,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
}

#[cfg(target_os = "windows")]
unsafe fn follow_hwnd(hwnd: sys::Hwnd) -> bool {
    use std::ptr;
    use sys::*;

    let hr = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);
    // S_FALSE / RPC_E_CHANGED_MODE 都表示"COM 已经就绪"，继续用
    if hr < 0 && hr != RPC_E_CHANGED_MODE {
        log::warn!("[桌面] COM 初始化失败（hr={:#x}），跳过跟随", hr);
        return false;
    }

    let mut obj: *mut std::ffi::c_void = ptr::null_mut();
    let hr = CoCreateInstance(&CLSID_VDM, ptr::null_mut(), CLSCTX_ALL, &IID_VDM, &mut obj);
    if hr < 0 || obj.is_null() {
        log::warn!("[桌面] 拿不到 IVirtualDesktopManager（hr={:#x}），跳过跟随", hr);
        return false;
    }
    // COM 对象的头一个字段就是虚函数表指针
    let vt = *(obj as *mut *const Vtable);
    let (is_on_current, get_desktop_id, move_to_desktop, release) = (
        (*vt).is_on_current,
        (*vt).get_desktop_id,
        (*vt).move_to_desktop,
        (*vt).release,
    );

    // 1. 已经在当前桌面 → 啥也不用干
    let mut on_current: i32 = 0;
    let hr = is_on_current(obj, hwnd, &mut on_current);
    if hr < 0 {
        log::warn!("[桌面] 查不到窗口在哪个桌面（hr={:#x}），跳过跟随", hr);
        release(obj);
        return false;
    }
    if on_current != 0 {
        release(obj);
        return false;
    }

    // 2. 当前桌面是哪个：**问前台窗口要** —— 前台窗口必然在当前这个桌面上。
    //    拿不到前台窗口就退一步问系统外壳（桌面 / 任务栏），它同样在当前桌面。
    let probe = GetForegroundWindow();
    let probe = if probe.is_null() { GetShellWindow() } else { probe };
    if probe.is_null() {
        log::warn!("[桌面] 找不到能问路的窗口，跳过跟随");
        release(obj);
        return false;
    }
    let mut id = Guid::default();
    let hr = get_desktop_id(obj, probe, &mut id);
    if hr < 0 || id == Guid::default() {
        log::warn!("[桌面] 查不到当前桌面（hr={:#x}），跳过跟随", hr);
        release(obj);
        return false;
    }

    // 3. 搬过去
    let hr = move_to_desktop(obj, hwnd, &id);
    release(obj);
    if hr < 0 {
        log::warn!("[桌面] 搬家失败（hr={:#x}），窗口留在原桌面", hr);
        return false;
    }
    log::info!("[桌面] 窗口不在当前虚拟桌面，已搬过来（桌面 {:08x}）", id.d1);
    true
}

#[cfg(target_os = "windows")]
unsafe fn probe_hwnd(hwnd: sys::Hwnd) {
    use std::ptr;
    use sys::*;

    let hr = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);
    if hr < 0 && hr != RPC_E_CHANGED_MODE {
        log::warn!("[桌面] 自检：COM 初始化失败（hr={:#x}）", hr);
        return;
    }
    let mut obj: *mut std::ffi::c_void = ptr::null_mut();
    let hr = CoCreateInstance(&CLSID_VDM, ptr::null_mut(), CLSCTX_ALL, &IID_VDM, &mut obj);
    if hr < 0 || obj.is_null() {
        log::warn!("[桌面] 自检：拿不到 IVirtualDesktopManager（hr={:#x}），跨桌面跟随用不了", hr);
        return;
    }
    let vt = *(obj as *mut *const Vtable);
    let mut on_current: i32 = 0;
    let hr1 = ((*vt).is_on_current)(obj, hwnd, &mut on_current);
    let probe = GetForegroundWindow();
    let probe = if probe.is_null() { GetShellWindow() } else { probe };
    let mut id = Guid::default();
    let hr2 = if probe.is_null() {
        -1
    } else {
        ((*vt).get_desktop_id)(obj, probe, &mut id)
    };
    ((*vt).release)(obj);
    log::info!(
        "[桌面] 自检：接口可用；窗口在当前桌面={}（hr={:#x}）；当前桌面={:08x}（hr={:#x}）",
        on_current != 0,
        hr1,
        id.d1,
        hr2
    );
}

#[cfg(target_os = "windows")]
unsafe fn hide_taskbar_hwnd(hwnd: sys::Hwnd) -> bool {
    use sys::*;

    let cur = GetWindowLongW(hwnd, GWL_EXSTYLE);
    let next = (cur & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW;
    if next == cur {
        return false; // 已经是这个状态，不用重复动
    }
    SetWindowLongW(hwnd, GWL_EXSTYLE, next);
    // 改完必须让系统重画一次边框：**少了这步，样式改了也不生效**
    SetWindowPos(
        hwnd,
        std::ptr::null_mut(),
        0,
        0,
        0,
        0,
        SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
    log::info!(
        "[任务栏] 已从任务栏摘掉（exStyle {:#x} → {:#x}）",
        cur as u32,
        next as u32
    );
    true
}
