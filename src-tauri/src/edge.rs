//! 贴边隐藏：窗口拖到屏幕左/右边缘停住后自动收进去（只露一条缝），
//! 鼠标碰到那条缝又滑出来。
//!
//! 屏幕是金贵的，清单不该长期占着一块地方 —— 这个用法是从 QQ / 飞书那儿学来的。
//!
//! **五个不自知的坑**（都试出来了，别改回去）：
//!   1. 收起时**只是把窗口移到屏幕外沿，绝不 `hide()`**。完全隐藏后 WebView
//!      收不到任何鼠标事件，窗口就再也滑不出来，只能去托盘点。
//!   2. 收起状态下的坐标**绝不能写进 config**（`lib.rs` 的 `save_window_state`
//!      已经拦了），否则下次启动窗口落在屏幕外，看着像"打不开"。
//!   3. "停住了"要在一段时间**之后再**判定：`Moved` 事件在拖动过程中是连发的，
//!      在事件里直接判定会在拖到一半就把窗口收走。所以这里用「最后移动时间 + 静默时长」。
//!   4. **鼠标还在窗口里时就别收**。否则刚滑出来 400 毫秒又被收走，
//!      用户伸手去点，窗口正好消失 —— 这是"看起来随机发疯"的典型来源。
//!   5. **动画期间要屏蔽自动收起**：动画自己在动窗口，会连发 `Moved` 事件，
//!      守护线程会把这堆事件当成"用户刚拖过"，两边互相打断。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, LogicalPosition, Manager, WebviewWindow};

/// 收起来之后还露在外面多少像素 —— 鼠标得有东西可以撞到
const PEEK: f64 = 5.0;
/// 鼠标贴到屏幕边缘多少像素以内算"碰到缝了"。
///
/// 拿**屏幕边缘**当参照物，而不是窗口自己在屏幕外的坐标 —— 那个坐标一会儿虚拟一会儿物理，
/// 很容易算出个鼠标永远到不了的判定区。屏幕边缘是绝对的，缝就长在上面。
///
/// **这个宽度是"误触半径"，能小就小**：40（约 32 个逻辑像素）时，鼠标只是路过屏幕边缘
/// —— 去点旁边的窗口、去够任务栏 —— 也会把收着的窗口带出来，而带出来之后还要等冷却期
/// 才收得回去，看着就像窗口自己在发疯。收窄到 20（约 16 个逻辑像素）：
/// 真要叫它出来的人会把鼠标推到头（那时 mx 几乎就是 0），照样一撞就中；
/// 顺路经过的人多半在十几像素以外，不会碰到。
const SLACK: f64 = 20.0;

/// 鼠标在缝里**停够这么久**才滑出来 —— 防误触主要靠这一档。
///
/// 光是"压在缝上"不算数：鼠标从屏幕边缘扫过去时也会短暂落进判定区，
/// 那不是"我要用它"。停顿是**意图**的信号，扫过是**路过**，两者必须分开。
/// 400ms 是"停一下"的量级：刻意去碰它的人几乎无感，路过的人等不到。
const PEEK_DWELL_MS: u64 = 400;
/// 离边缘多近算"贴边"。物理像素，且要留足余量：
/// 14 的时候约等于 11 个逻辑像素，手稍微抖一点就不算贴边，用户会觉得"时灵时不灵"。
const SNAP: f64 = 24.0;
/// 松手多久算停住了
const SETTLE_MS: u64 = 400;
/// 滑出后的冷却期：这段时间内一律不许自动收起。
///
/// **为什么必需**：滑出动画刚跑完的那一瞬间，窗口还站在屏幕外沿（动画起点），
/// 而鼠标贴在屏幕边上 —— 拿这个瞬间去判定，结论会是"窗口没贴边/鼠标不在窗口里"，
/// 于是刚滑出来就被吞回去。冷却期从"滑出"这个动作本身算起，不看坐标，最不容易错。
const REVEAL_COOLDOWN_MS: u64 = 900;
/// 鼠标离开窗口后的宽限期：这么久了还不在窗口里，才允许收起。
///
/// **为什么不"一离开就收"**：滑出来之后鼠标要往窗口里挪，这一路上很容易擦出窗口边界，
/// 立刻收会把"正要用它"的动作变成"刚要碰它就没了"。给一点缓冲，手感才不神经质。
const LEAVE_GRACE_MS: u64 = 500;

/// 主动唤起之后，最多等多久"鼠标进来一次"。
///
/// **为什么不能无限等**：`AWAIT_HOVER` 的本意只是挡住"刚叫出来就被吞回去"那一瞬，
/// 可它只有"鼠标进过窗口"才会被清掉 —— 用快捷键叫出来、鼠标一直停在别的窗口上，
/// 这个标记就**永远卡着**，窗口从此再也不自动收（"鼠标不在窗口里它也不收"多半就是它）。
/// 冷却期（`REVEAL_COOLDOWN_MS`）已经挡住了刚滑出那一瞬，这里给一个有上限的宽限就够了。
const AWAIT_HOVER_MAX_MS: u64 = 5000;

/// `ANIMATING` 最多能挂多久（正常动画最长的 `REVEAL_MS` 也才 160ms）。
///
/// **为什么需要兜底**：动画线程在 `set_position` 失败时会提前 return，
/// 那次就没机会把 `ANIMATING` 清回去 —— 标记一卡住，**之后永远收不起来**，
/// 而且界面上一点异常都没有（看起来就是"贴边突然失灵了"）。
/// 照 `HIDDEN` 那条老教训办：**缓存来的状态，必须能被真相（时间）校正。**
const ANIM_MAX_MS: u64 = 2000;

/// 启动后这么久之内不自动收起（见 `check_settle` 里那条判定的说明）
const BOOT_GRACE_MS: u64 = 3000;

/// "贴着边却没收回"的日志节流：守护线程每 120ms 跑一次，
/// 同一种原因最多这么多毫秒记一条，否则日志会被刷满。
const SKIP_LOG_MS: u64 = 5000;

/// 鼠标离窗口多远算"明显是走开了"（物理像素）。
///
/// 宽限期只用来饶过"擦出边界"这种贴身而过的情况。鼠标都跑到一屏之外了还等它，
/// 就变成"我早就不看它了它却赖着不收"。所以离得远就立刻收，不跟宽限期商量。
const FAR_AWAY: f64 = 150.0;

/// 动画：滑出比收起略慢一点 —— 出来的东西要让人看清它从哪来，收走干脆些就行
const REVEAL_MS: u64 = 160;
const HIDE_MS: u64 = 140;
/// 动画每帧间隔（约 60fps）
const FRAME_MS: u64 = 16;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Side {
    Left,
    Right,
}

/// 当前处于收起状态时的那一边（None = 正常显示）
static HIDDEN: Mutex<Option<Side>> = Mutex::new(None);
/// 最后一次移动窗口的时刻（毫秒；0 = 本次运行没动过）
static LAST_MOVE_MS: AtomicU64 = AtomicU64::new(0);
/// 最后一次滑出的时刻（毫秒）。见 `REVEAL_COOLDOWN_MS`
static LAST_REVEAL_MS: AtomicU64 = AtomicU64::new(0);
/// 最后一次"鼠标在窗口里"的时刻（毫秒）。见 `LEAVE_GRACE_MS`
static LAST_INSIDE_MS: AtomicU64 = AtomicU64::new(0);
/// 鼠标**连续**压在缝上的起始时刻（毫秒；0 = 此刻没在缝上）。见 `PEEK_DWELL_MS`
static PEEK_SINCE_MS: AtomicU64 = AtomicU64::new(0);
/// 唤起之后是否还在等"鼠标进来过一次"。
///
/// **为什么需要**：从任务栏 / 托盘 / 快捷键把窗口叫出来时，鼠标还停在任务栏或托盘上，
/// 根本不在窗口里 —— 按常规判定"贴边 + 鼠标不在窗口内 → 收"，窗口刚滑出来就被吞回去，
/// 用户看到的就是"点了没用"。所以**主动唤起之后，必须先等鼠标进来过一次才恢复自动收起**：
/// 那一下才是"我确实要用它"的信号。
static AWAIT_HOVER: AtomicBool = AtomicBool::new(false);
/// 唤起（托盘 / 快捷键 / 任务栏）的时刻。见 `AWAIT_HOVER_MAX_MS`
static AWAIT_HOVER_SINCE_MS: AtomicU64 = AtomicU64::new(0);
static ENABLED: AtomicBool = AtomicBool::new(true);
/// 正在播动画时不许自动收起，见坑 5
static ANIMATING: AtomicBool = AtomicBool::new(false);
/// 这一段动画从什么时候开始。见 `ANIM_MAX_MS` —— 用来把卡住的标记拽回来
static ANIM_START_MS: AtomicU64 = AtomicU64::new(0);
/// 动画代号：每次开新动画就 +1，旧动画发现自己过期了要立刻收手（让位给新的）
static ANIM_SEQ: AtomicU64 = AtomicU64::new(0);
/// "贴着边却没收回"的日志节流：上次记的原因 + 时刻
static LAST_SKIP: Mutex<(u64, String)> = Mutex::new((0, String::new()));
/// 进程（守护线程）起跑的时刻。见 `BOOT_GRACE_MS`
static BOOT_MS: AtomicU64 = AtomicU64::new(0);

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

pub fn is_hidden() -> bool {
    HIDDEN.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// `Moved` / `Resized` 事件里调用一下，记下"刚动过"
pub fn mark_moved() {
    LAST_MOVE_MS.store(now_ms(), Ordering::SeqCst);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 窗口压着的那块屏幕的水平范围（物理像素）：返回 (left, width)
///
/// **必须按"交集最大"来找，不能按"左上角落在谁家里"**：窗口收起后大半在屏幕外，
/// 左上角坐标是 -461 这种值，一块显示器都匹配不上 —— 滑出就算不出目标位置，
/// 窗口会永远出不来（这是"收起来就再也不出来"的第二个原因）。
fn monitor_bounds(win: &WebviewWindow, x: f64, y: f64, w: f64, h: f64) -> Option<(f64, f64)> {
    let monitors = win.available_monitors().ok()?;
    let mut best: Option<(f64, f64, f64)> = None; // (交集面积, left, width)
    for m in monitors.iter() {
        let (ml, mt) = (m.position().x as f64, m.position().y as f64);
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        let ox = (x + w).min(ml + mw) - x.max(ml);
        let oy = (y + h).min(mt + mh) - y.max(mt);
        if ox <= 0.0 || oy <= 0.0 {
            continue; // 跟这块屏没有交集
        }
        let area = ox * oy;
        if best.map(|b| area > b.0).unwrap_or(true) {
            best = Some((area, ml, mw));
        }
    }
    if let Some((_, left, width)) = best {
        return Some((left, width))
    }
    // 一块屏都交不上 = 窗口整个在屏幕外面（被拖出去就是这样）。
    // 这时按"中心离哪块屏最近"来认 —— 不然连"它贴的是哪一边"都算不出来，
    // 结果就是**拖到屏幕外反而不收**（左侧能收是因为它还留着一条边在里面，右侧整个在外面）。
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let mut nearest: Option<(f64, f64, f64)> = None; // (距离平方, left, width)
    for m in monitors.iter() {
        let (ml, mt) = (m.position().x as f64, m.position().y as f64);
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        let d = (cx - (ml + mw / 2.0)).powi(2) + (cy - (mt + mh / 2.0)).powi(2);
        if nearest.map(|n| d < n.0).unwrap_or(true) {
            nearest = Some((d, ml, mw));
        }
    }
    nearest.map(|n| (n.1, n.2))
}

// ---- 滑出 / 收起，以及它们的动画 ----

/// 窗口现在能不能参与贴边判定（可见、且没被最小化）。
///
/// **最小化必须排除**，这是个会咬人的坑：Windows 把最小化窗口的位置设成 `(-32000, -32000)`，
/// 而贴边判定**只看坐标** —— 于是"窗口被最小化"会被读成"被拖到屏幕左侧外面三万像素"，
/// 守护线程立刻去把它挪到贴边位置（日志里的原话：`x -32000 → -155`）。
/// 用户看到的现象是：**最小化再恢复，窗口跑到屏幕边上了**。
///
/// 这个值和"收起状态不写进 config"防的是同一件事，两处都得认它。
fn window_in_play(win: &WebviewWindow) -> bool {
    if !win.is_visible().unwrap_or(false) {
        return false;
    }
    if win.is_minimized().unwrap_or(false) {
        return false;
    }
    true
}

/// 收进边缘。窗口本来就站在别的地方时不做任何事 —— 免得鼠标一离开就被收走
pub fn hide_to_edge(app: &AppHandle) -> bool {
    if is_hidden() || !is_enabled() {
        return false;
    }
    let win = match app.get_webview_window("main") {
        Some(w) => w,
        None => return false,
    };
    if !window_in_play(&win) {
        return false;
    }
    // 正在播动画时不收：**动画自己在挪窗口**，这期间任何位置/进出判定都不可信（坑 5）。
    // 少了这一条，滑出动画刚跑完就被"贴边"判定收回去 —— 表现为刚出来又没了。
    if ANIMATING.load(Ordering::SeqCst) {
        return false;
    }
    // **鼠标此刻在窗口里就别收**，哪怕这次是前端"鼠标离开过"那条路触发的。
    //
    // 为什么必须在这里再查一次：滑出动画期间窗口自己在动，鼠标**相对窗口**会进进出出，
    // 前端的 `mouseleave` 会误报 —— 定时器一到就调过来收，而此时鼠标正压在窗口上。
    // 收起来之后鼠标还贴着那条缝，下一拍又被碰出来，于是"滑出→收起→滑出"抖动个不停。
    // 判定只信实时光标位置（理由见 `pointer_in_window` 的注释）。
    if pointer_in_window(&win) {
        LAST_INSIDE_MS.store(now_ms(), Ordering::SeqCst);
        return false;
    }
    // 刚被唤起、鼠标还没进来过 → 用户在别处（任务栏 / 托盘），留着别收（见 AWAIT_HOVER）
    if AWAIT_HOVER.load(Ordering::SeqCst) {
        return false;
    }
    // 最大化状态下窗口左右边缘本来就是贴着屏幕的，会被误判成"贴边"—— 不管它
    if win.is_maximized().unwrap_or(false) {
        return false;
    }
    // **必须用 `outer_size` 而不是 `inner_size`**：系统是按整个窗口（含边框）摆放的，
    // 拿客户区宽度去算"露 5px"，实际会露出二十来像素。
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return false,
    };
    let (x, w) = (pos.x as f64, size.width as f64);
    let (left, mw) = match monitor_bounds(&win, x, pos.y as f64, w, size.height as f64) {
        Some(v) => v,
        None => return false,
    };

    let gap_left = x - left;
    let gap_right = left + mw - (x + w);
    let side = if gap_left <= SNAP && gap_left <= gap_right {
        Side::Left
    } else if gap_right <= SNAP {
        Side::Right
    } else {
        return false; // 没贴边，不动
    };

    let to_x = match side {
        Side::Left => left - w + PEEK,
        Side::Right => left + mw - PEEK,
    };
    // **先标记成收起态再动画**：标记一生效，`save_window_state` 就不会把收起过程中的
    // 中间坐标写进 config（写进去下次启动窗口就落在屏幕外了）。
    if let Ok(mut g) = HIDDEN.lock() {
        *g = Some(side);
    }
    // 收起那一刻鼠标多半还贴在边上，缝上的计时必须清零 ——
    // 否则会把"收起前那一段停留"接着算下去，刚收进去就又滑出来
    PEEK_SINCE_MS.store(0, Ordering::SeqCst);
    log::info!(
        "[贴边] 贴住 {:?} 边，收起来（x {:.0} → {:.0}）",
        side,
        x,
        to_x
    );
    animate_to(app, x, to_x, pos.y as f64, HIDE_MS, false);
    true
}

/// 从边缘滑出来，停在贴边的那个位置。不是收起状态时啥也不做（幂等）
///
/// `wait_for_hover`：滑出之后要不要"等鼠标进来过一次"才恢复自动收起。
///
/// **这两条路必须分开**（合在一起就是"划过边上再快速划走就不收回"）：
/// - **从别处唤起**（托盘 / 快捷键 / 任务栏 / 第二次启动）：鼠标必然还在别处，
///   不等一下就会被"贴边 + 鼠标不在窗口"立刻收回去，用户看到的是"点了没用" → **要等**；
/// - **鼠标碰缝滑出来**：鼠标**此刻就在边上**，不存在"我刚叫它出来它就被吞"的问题 → **不等**。
///   要是也等，鼠标一划而过时守护线程（120ms 一拍）往往压根没看见它进过窗口，
///   那个标记就一直挂着不收 —— 表现就是"非要刻意停一会儿再离开才收"。
pub fn reveal(app: &AppHandle, wait_for_hover: bool) -> bool {
    let side = match HIDDEN.lock() {
        Ok(mut g) => g.take(),
        Err(_) => return false,
    };
    let side = match side {
        Some(s) => s,
        None => return false,
    };
    // 收着的窗口可能留在另一个虚拟桌面（上次是在那边贴的边）。
    // 鼠标是在**当前**桌面的边上停够了才走到这儿 —— 先跟过来，再滑出来。
    // 少了这一步，它会在看不见的那个桌面里悄悄滑出来，用户以为"碰了没反应"。
    crate::desktop::follow_current_desktop(app);
    let win = match app.get_webview_window("main") {
        Some(w) => w,
        None => return false,
    };
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return false,
    };
    let (left, mw) = match monitor_bounds(
        &win,
        pos.x as f64,
        pos.y as f64,
        size.width as f64,
        size.height as f64,
    ) {
        Some(v) => v,
        None => {
            log::warn!("[贴边] 滑出失败：找不到窗口所在的那块屏");
            return false;
        }
    };
    let to_x = match side {
        Side::Left => left,
        Side::Right => left + mw - size.width as f64,
    };
    // 这里不用记"鼠标在窗口里"：守护线程每步都实时问系统光标位置（见 `pointer_in_window`）。
    // 但要记下"刚滑出来过"，冷却期内不许再收 —— 否则动画跑完那一瞬间窗口还在屏幕外沿，
    // 判定会认为"没贴边"，刚出来就被吞回去（表现为闪一下又没了）。
    LAST_REVEAL_MS.store(now_ms(), Ordering::SeqCst);
    // 见 `reveal` 上那条注释：只有"从别处唤起"才需要等鼠标进来一次，
    // 且这个等待**有上限**（AWAIT_HOVER_MAX_MS），不能无限等
    if wait_for_hover {
        AWAIT_HOVER.store(true, Ordering::SeqCst);
        AWAIT_HOVER_SINCE_MS.store(now_ms(), Ordering::SeqCst);
    } else {
        AWAIT_HOVER.store(false, Ordering::SeqCst);
        AWAIT_HOVER_SINCE_MS.store(0, Ordering::SeqCst);
    }
    log::info!("[贴边] 滑出（{:?} 边，x {:.0} → {:.0}）", side, pos.x, to_x);
    animate_to(app, pos.x as f64, to_x, pos.y as f64, REVEAL_MS, true);
    true
}

/// 把窗口从 `from_x` 平滑挪到 `to_x`（横着动，纵向不动）。立刻返回，动画在独立线程里跑。
///
/// 为什么不在调用线程里播完：这条路会被 invoke 触发，占着线程会让整个 IPC 排队等着。
fn animate_to(app: &AppHandle, from_x: f64, to_x: f64, y: f64, ms: u64, focus_at_end: bool) {
    let seq = ANIM_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    ANIMATING.store(true, Ordering::SeqCst);
    // 记下起跑时刻：`ANIMATING` 是个缓存，得有办法被"它挂太久了"这个真相校正
    ANIM_START_MS.store(now_ms(), Ordering::SeqCst);
    let handle = app.clone();
    let steps = (ms / FRAME_MS).max(1);

    thread::spawn(move || {
        for i in 1..=steps {
            // 有更新的动画接管了 —— 立刻放手，别两个线程抢着挪同一个窗口
            if ANIM_SEQ.load(Ordering::SeqCst) != seq {
                return;
            }
            let Some(win) = handle.get_webview_window("main") else {
                return;
            };
            let scale = win.scale_factor().unwrap_or(1.0);
            let progress = ease_out(i as f64 / steps as f64);
            let x = from_x + (to_x - from_x) * progress;
            if win
                .set_position(LogicalPosition {
                    x: x / scale,
                    y: y / scale,
                })
                .is_err()
            {
                return;
            }
            thread::sleep(Duration::from_millis(FRAME_MS));
        }
        // 只有"最后一个没被接管的自己"才有权把这些状态收回去
        if ANIM_SEQ.load(Ordering::SeqCst) == seq {
            ANIMATING.store(false, Ordering::SeqCst);
            ANIM_START_MS.store(0, Ordering::SeqCst);
            if focus_at_end {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.set_focus();
                }
            }
        }
    });
}

/// 先快后慢（三次缓出）：起步要有劲，收尾要稳，看着才像"被推出来"而不是匀速滑
fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// 鼠标此刻在屏幕上的位置（物理像素）。
///
/// **为什么要亲自问系统**：窗口收起后露在外面只有 PEEK 那么宽，而无边框窗口周围
/// 还有一圈 8px 的缩放热区（摸不到网页），那条细缝**整体都落在收不到鼠标事件的区域里**
/// —— 网页的 `mouseenter` 根本不会触发，窗口就再也滑不出来。只能直接查全局光标位置。
fn cursor_position() -> Option<(f64, f64)> {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }
        #[link(name = "user32")]
        extern "system" {
            fn GetCursorPos(p: *mut POINT) -> i32;
        }
        let mut p = POINT { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut p) != 0 };
        if ok {
            return Some((p.x as f64, p.y as f64))
        }
        return None
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 鼠标此刻是否落在窗口矩形内 —— 直接问系统要光标位置。
///
/// **为什么不听网页的 `mouseenter` / `mouseleave`**：那两个事件会丢。鼠标被程序挪动、
/// 窗口自己滑动、光标在边界上闪烁，都会漏掉一次 `mouseleave`，于是"鼠标还在窗口里"
/// 这个标记就**永远停在 true**，窗口从此再也不自动收起 —— 表现为"贴边功能时灵时不灵"。
/// 光标位置本来就查得到，实时问一次最可靠，也让判定和 `poll_peek` 用同一个数据源。
fn pointer_in_window(win: &WebviewWindow) -> bool {
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return false,
    };
    let (mx, my) = match cursor_position() {
        Some(v) => v,
        None => return false,
    };
    let (x, y) = (pos.x as f64, pos.y as f64);
    mx >= x && mx <= x + size.width as f64 && my >= y && my <= y + size.height as f64
}

/// 鼠标离窗口矩形最近有多远（物理像素）。鼠标在窗口里时为 0。
///
/// 用来把两种情况分开：**擦着边过去**（要给宽限，别神经过敏）
/// 和**人已经走开了**（该立刻收，而不是还等宽限期）。
fn distance_to_window(win: &WebviewWindow) -> f64 {
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return 0.0,
    };
    let (mx, my) = match cursor_position() {
        Some(v) => v,
        None => return 0.0,
    };
    let (x, y) = (pos.x as f64, pos.y as f64);
    // 把鼠标夹进窗口矩形，得到矩形上离它最近的那个点
    let cx = mx.clamp(x, x + size.width as f64);
    let cy = my.clamp(y, y + size.height as f64);
    ((mx - cx).powi(2) + (my - cy).powi(2)).sqrt()
}

/// 把"已收起"标记跟窗口的真实位置对一次账。
///
/// **为什么必需**：收起之后，用户可以从露出的那几像素按住标题栏把窗口拖出来。
/// 这时窗口已经回到屏幕里了，可标记还写着"收着呢" —— 标记一卡住，后面全短路：
///   · `check_settle` 头一行就返回 → **之后贴边再也不收起**；
///   · `poll_peek` 照旧盯着屏幕边缘 → 鼠标一蹭就把窗口猛地吸回边上（看着像"被吞回去"）。
///
/// 与其去堵每一条能把窗口挪走的路径（拖动、系统调整、分辨率变化……），不如每次轮询都用
/// 窗口自己的位置校正一次 —— **位置是唯一的真相**，标记只是它的缓存。
fn sync_hidden_state(win: &WebviewWindow) {
    if !is_hidden() {
        return;
    }
    // 最小化时坐标是 (-32000,-32000)，拿它算"露出多少"毫无意义（会算出个巨大的负数）
    if !window_in_play(win) {
        return;
    }
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return,
    };
    let (left, mw) = match monitor_bounds(
        win,
        pos.x as f64,
        pos.y as f64,
        size.width as f64,
        size.height as f64,
    ) {
        Some(v) => v,
        None => return,
    };
    // 窗口落在屏幕里的那一段有多宽
    let vis_left = (pos.x as f64).max(left);
    let vis_right = (pos.x as f64 + size.width as f64).min(left + mw);
    let visible = vis_right - vis_left;
    // 收起状态的特征是"只在屏幕里露出 PEEK 那么多"。
    //   · 露出明显更多 → 被弄回屏幕里了；
    //   · 一点也不露（负数）→ 整个跑到屏幕外去了。
    // **两种都不算收起状态**，标记都得作废。尤其是后一种：窗口被拖到屏幕外时标记若还留着，
    // `check_settle` 会一直提前返回，它就永远收不起来了（"拖出去也不收"两次都是这么来的）。
    let at_edge = visible > 0.0 && visible <= PEEK * 3.0;
    if !at_edge {
        log::info!(
            "[贴边] 窗口不在收起位置（露出 {:.0}px），清掉收起标记",
            visible
        );
        if let Ok(mut g) = HIDDEN.lock() {
            *g = None;
        }
    }
}

/// 鼠标此刻是不是正压在那条露出来的缝上（是的话返回那一边）。
///
/// **只判定，不改状态** —— "碰到缝"和"想把它叫出来"是两件事，计时交给 `poll_peek`。
/// 分开之后，"路过"和"停住"才有地方区分；原来的写法一碰到就滑出来，
/// 于是鼠标扫过屏幕边缘也会把它带出来（等冷却期过了才收得回去，看着像发疯）。
fn peek_hit(app: &AppHandle) -> Option<Side> {
    let win = match app.get_webview_window("main") {
        Some(w) => w,
        None => return None,
    };
    // 最小化的窗口坐标是 (-32000,-32000)，看着像"贴在屏幕左外" —— 别去理它
    if !window_in_play(&win) {
        return None;
    }
    // 先对账：窗口要是已经被拖回屏幕里了，标记就该作废 ——
    // 否则会盯着屏幕边缘把好端端摆着的窗口猛地吸回去
    sync_hidden_state(&win);
    let side = match HIDDEN.lock() {
        Ok(g) => *g,
        Err(_) => return None,
    };
    let side = match side {
        Some(s) => s,
        None => return None, // 现在是正常显示的，不用管
    };
    let (mx, my) = match cursor_position() {
        Some(v) => v,
        None => return None,
    };
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return None,
    };
    let (x, top) = (pos.x as f64, pos.y as f64);
    let bottom = top + size.height as f64;
    if my < top || my > bottom {
        return None; // 鼠标不在这条竖带所在的高度上
    }
    // **判定用屏幕边缘，不用窗口自己的坐标**：
    // 收起来时窗口站在屏幕外面（x 是 -461 或 2773 这种），拿它的坐标去算"缝在哪"，
    // 只要坐标系有一点没对齐（虚拟 / 物理、含不含边框），算出来的判定区就没鼠标能碰到
    // —— "收起来就再也不出来"就是这么来的。屏幕边缘是绝对参照物，绕开这个坑。
    let (left, mw) = match monitor_bounds(
        &win,
        x,
        pos.y as f64,
        size.width as f64,
        size.height as f64,
    ) {
        Some(v) => v,
        None => return None,
    };
    let hit = match side {
        Side::Left => mx <= left + SLACK,
        Side::Right => mx >= left + mw - SLACK,
    };
    if hit {
        Some(side)
    } else {
        None
    }
}

/// 守护线程每步顺带做的事：鼠标在缝上**停够了** → 滑出来。
///
/// 判定由 Rust 自己来做，不去指望网页事件（原因见 `cursor_position` 的注释）。
pub fn poll_peek(app: &AppHandle) {
    let Some(side) = peek_hit(app) else {
        // 离开缝（或者根本没在缝上）→ 计时清零。下次再碰必须重新停够，
        // 不能把几次路过的碎片时间攒起来凑数
        PEEK_SINCE_MS.store(0, Ordering::SeqCst);
        return;
    };
    let now = now_ms();
    let since = PEEK_SINCE_MS.load(Ordering::SeqCst);
    if since == 0 {
        // 刚碰上：只记个时刻，先不动 —— 停得住才说明是真要它出来
        PEEK_SINCE_MS.store(now, Ordering::SeqCst);
        return;
    }
    if now - since < PEEK_DWELL_MS {
        return; // 还在等，看你是停在这儿还是路过
    }
    PEEK_SINCE_MS.store(0, Ordering::SeqCst);
    log::info!("[贴边] 鼠标在 {:?} 边的缝上停够了（{}ms），滑出来", side, PEEK_DWELL_MS);
    // 鼠标就在边上碰出来的：**不等**它再进来一次（详见 reveal 的注释）
    let _ = reveal(app, false);
}

// ---- 守护线程 ----

/// 守护线程每一步做的事：窗口停下、贴在竖边上、鼠标又不在里头 → 收起来。
///
/// 放在独立线程里轮询，而不是挂在 `Moved` 事件上 —— 事件连发时没法判断"这次算不算停住了"。
pub fn check_settle(app: &AppHandle) {
    if !is_enabled() {
        return;
    }
    let win = match app.get_webview_window("main") {
        Some(w) => w,
        None => return,
    };
    // 先对账：窗口被拖回屏幕里的话，收起标记就该作废 ——
    // 不然 `is_hidden()` 一直是真，这里永远提前返回，**之后贴边再也不会自动收起**。
    sync_hidden_state(&win);
    if is_hidden() {
        return; // 还收着呢，等它被滑出来
    }
    // 最大化窗口的左右边缘天然贴在屏幕上，那不算"用户把窗口拖到了边上"
    if win.is_maximized().unwrap_or(false) {
        return;
    }
    if !window_in_play(&win) {
        return;
    }
    let (pos, size) = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return,
    };
    let (x, w) = (pos.x as f64, size.width as f64);
    let (left, mw) = match monitor_bounds(&win, x, pos.y as f64, w, size.height as f64) {
        Some(v) => v,
        None => return,
    };
    // **带符号的距离，不能取绝对值**：窗口被拖到屏幕外时 x 是 -600 这种，
    // `abs()` 会把它算成"离屏幕边缘六百像素"从而判定成"没贴边"，
    // 于是"拖出去它也不收"。负数在这里的含义是"已经越过边界了"，正是最该收起的情况。
    let gap_left = x - left;
    let gap_right = left + mw - (x + w);
    // **先把"贴没贴边"算出来再往下走**：只有"贴着边却没收回"才值得记日志 ——
    // 窗口好端端摆中间不收是天经地义，记它只会把日志刷满。
    if gap_left > SNAP && gap_right > SNAP {
        return;
    }

    // 动画期间不收（坑 5）。但 `ANIMATING` 是缓存，得能校正：
    // 见过它卡在 true 上（动画线程提前退出），之后**永远**收不起来。
    if ANIMATING.load(Ordering::SeqCst) {
        let started = ANIM_START_MS.load(Ordering::SeqCst);
        let hung = started > 0 && now_ms() - started > ANIM_MAX_MS;
        if !hung {
            skip_log("正在播动画");
            return;
        }
        log::warn!(
            "[贴边] 动画标记挂了 {}ms 没清（动画线程多半是提前退了），强制清掉",
            now_ms() - started
        );
        ANIMATING.store(false, Ordering::SeqCst);
        ANIM_START_MS.store(0, Ordering::SeqCst);
    }
    // 启动后头几秒不自动收：窗口上次就停在边上的话，刚打开就滑走太突然。
    //
    // **不再要求"本次运行拖过窗口"**。那条老规矩的出发点是"别自作主张"，
    // 可它有个致命副作用：窗口**本来就停在边上**、启动时按原位置恢复（尺寸位置
    // 和上次一样 → 系统根本不发 Moved 事件 → 标记永远为 0），于是**一直不收**，
    // 用户看到的就是"贴边功能不灵"。而该收不该收其实已经由另外两条说了算：
    // 贴着边（≤ SNAP）+ 鼠标不在窗口里 —— 够充分了。
    if BOOT_MS.load(Ordering::SeqCst) == 0 {
        BOOT_MS.store(now_ms(), Ordering::SeqCst);
    }
    if now_ms() - BOOT_MS.load(Ordering::SeqCst) < BOOT_GRACE_MS {
        skip_log("刚启动，先不收");
        return;
    }
    let last = LAST_MOVE_MS.load(Ordering::SeqCst);
    if last > 0 && now_ms() - last < SETTLE_MS {
        return; // 还在拖，这是正常情况，不用记
    }
    // 刚滑出来过：给它一点时间站稳，这期间谁说都不收
    let revealed = LAST_REVEAL_MS.load(Ordering::SeqCst);
    if revealed > 0 && now_ms() - revealed < REVEAL_COOLDOWN_MS {
        return; // 同上：冷却期是设计好的，不算异常
    }
    // 鼠标还在窗口里就别收（坑 4）—— 实时问系统，不听前端事件（见 `pointer_in_window`）
    if pointer_in_window(&win) {
        LAST_INSIDE_MS.store(now_ms(), Ordering::SeqCst);
        // 鼠标进来过了：撤掉"等鼠标"的标记，之后按常规判定
        AWAIT_HOVER.store(false, Ordering::SeqCst);
        return;
    }
    // 唤起之后鼠标还没进来过 → 用户在别处（任务栏 / 托盘），窗口留着别收。
    // **但不能无限等**（见 AWAIT_HOVER_MAX_MS）：等过头就按常规判定放行，
    // 否则标记一卡住，窗口就再也不收了 —— "鼠标不在窗口里它也不收"多半出在这儿。
    if AWAIT_HOVER.load(Ordering::SeqCst) {
        let since = AWAIT_HOVER_SINCE_MS.load(Ordering::SeqCst);
        if since > 0 && now_ms() - since < AWAIT_HOVER_MAX_MS {
            skip_log("唤起后还在等鼠标进来一次");
            return;
        }
        log::info!(
            "[贴边] 唤起后 {}ms 鼠标也没进来，不再等了，按常规判定",
            AWAIT_HOVER_MAX_MS
        );
        AWAIT_HOVER.store(false, Ordering::SeqCst);
    }
    // 鼠标刚离开一会儿：再等等（见 `LEAVE_GRACE_MS`）。
    // 没有这一条，鼠标从屏幕边缘往窗口里挪的路上擦出去一下，窗口就缩回去了。
    //
    // 但如果鼠标已经**离得很远**，说明人是真的走开了 —— 这时候还等宽限期，
    // 就成了"我早不看它了，它却赖着不收"。所以近处才给宽限。
    let inside = LAST_INSIDE_MS.load(Ordering::SeqCst);
    if inside > 0
        && now_ms() - inside < LEAVE_GRACE_MS
        && distance_to_window(&win) < FAR_AWAY
    {
        return; // 宽限期也是设计好的，不记
    }

    // 收之前把坐标摊开：这条日志是"为什么又收起来了"的唯一线索，
    // 鼠标和窗口的坐标必须看得见，才不用靠猜单位。
    if let Some((mx, my)) = cursor_position() {
        log::info!(
            "[贴边] 收起判定：鼠标({:.0},{:.0}) 窗口 x {:.0}~{:.0} y {:.0}~{:.0} 屏左 {:.0}",
            mx,
            my,
            x,
            x + w,
            pos.y as f64,
            pos.y as f64 + size.height as f64,
            left
        );
    }
    let _ = hide_to_edge(app);
}

/// "贴着边却没收"要在日志里说一声 —— 这是"贴边不灵"唯一的抓手。
///
/// 守护线程每 120ms 跑一次，不能每次都记：**同一种原因**最多 `SKIP_LOG_MS`
/// 记一条，否则日志会被这几行刷满，真正要看的东西反而看不见了。
fn skip_log(reason: &str) {
    let now = now_ms();
    let mut g = match LAST_SKIP.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    if g.1 == reason && now - g.0 < SKIP_LOG_MS {
        return;
    }
    *g = (now, reason.to_string());
    log::info!("[贴边] 贴着边但这次不收：{}", reason);
}

/// 守护线程的轮询间隔：比 `SETTLE_MS` 小得多，收起才跟手
pub const POLL_INTERVAL: Duration = Duration::from_millis(120);
