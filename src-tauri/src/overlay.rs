// src-tauri/src/overlay.rs

// =============================================================================
// 依赖引入
// =============================================================================
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use enigo::{Enigo, Mouse, Settings, Coordinate, Keyboard, Button, Direction, Key};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;

// =============================================================================
// 全局状态 —— 原子变量，多线程安全读写
// =============================================================================

/// 覆盖模式开关：true 时，快捷键执行自定义鼠标动作而非原按键
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 鼠标左右切换标志：用于 ToggleLeftRightCenter 动作，每次触发后翻转
static MOUSE_ON_LEFT: AtomicBool = AtomicBool::new(true);

/// 全局屏幕宽度（启动时根据实际显示器尺寸更新）
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(1920);

/// 全局屏幕高度（启动时根据实际显示器尺寸更新）
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(1080);

/// ★ 核心防护：正在执行"恢复（模拟原按键）"流程的标志
///
/// 当此标志为 true 时，全局快捷键回调检测到会直接 return，
/// 从而防止 enigo 模拟的原按键被自己的全局快捷键二次拦截形成重入。
///
/// 为什么用 AtomicBool 而不是 Mutex<bool>：
///   - 这个标志只在"恢复开始 → 模拟 → 恢复结束"期间被持有
///   - 由于一次只有一个快捷键会触发恢复，且整个过程在回调调用栈内完成，
///     不需要跨快捷键的互斥，AtomicBool 足够且无锁更快
///   - SeqCst 排序确保跨线程（OS 事件线程）的可见性
static RESTORING: AtomicBool = AtomicBool::new(false);

// =============================================================================
// 动作定义
// =============================================================================

/// 自定义鼠标动作（覆盖模式下执行）
pub enum MouseAction {
    /// 移动鼠标到绝对坐标 (x, y)
    MoveTo(i32, i32),
    /// 鼠标左键单击
    Click,
    /// 鼠标左键双击（间隔 50ms）
    DoubleClick,
    /// 移动到 (x, y) 后单击
    MoveToClick(i32, i32),
    /// 在屏幕左 1/4 和右 3/4 位置之间切换并单击
    ToggleLeftRightCenter,
}

/// 覆盖模式关闭时，模拟执行的原按键动作
pub enum RestoreAction {
    /// 模拟输入一段文本（如空格）
    TypeText(&'static str),
    /// 模拟按下并释放某个键盘按键
    SimulateKey(Key),
}

// =============================================================================
// 鼠标动作执行
// =============================================================================

/// 执行自定义鼠标动作（覆盖模式下调用）
fn execute_mouse_action(action: &MouseAction) {
    let sw = SCREEN_WIDTH.load(Ordering::Relaxed) as i32;
    let sh = SCREEN_HEIGHT.load(Ordering::Relaxed) as i32;

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        match action {
            MouseAction::MoveTo(x, y) => {
                let _ = enigo.move_mouse(*x, *y, Coordinate::Abs);
            }
            MouseAction::Click => {
                let _ = enigo.button(Button::Left, Direction::Click);
            }
            MouseAction::DoubleClick => {
                // 两次点击间隔 50ms，模拟双击
                let _ = enigo.button(Button::Left, Direction::Click);
                std::thread::sleep(Duration::from_millis(50));
                let _ = enigo.button(Button::Left, Direction::Click);
            }
            MouseAction::MoveToClick(x, y) => {
                // 先移动再点击
                let _ = enigo.move_mouse(*x, *y, Coordinate::Abs);
                let _ = enigo.button(Button::Left, Direction::Click);
            }
            MouseAction::ToggleLeftRightCenter => {
                // 根据 MOUSE_ON_LEFT 标志在屏幕左右 1/4 位置间切换
                let x = if MOUSE_ON_LEFT.load(Ordering::Relaxed) {
                    sw / 4
                } else {
                    sw * 3 / 4
                };
                let y = sh / 2;
                let _ = enigo.move_mouse(x, y, Coordinate::Abs);
                let _ = enigo.button(Button::Left, Direction::Click);

                // 翻转标志，下次切换到另一边
                MOUSE_ON_LEFT.fetch_xor(true, Ordering::Relaxed);
            }
        }
    }
}

// =============================================================================
// 核心：安全的「注销 → 模拟 → 重注册」流程
// =============================================================================
//
// 设计要点：
//
//   1. 此函数不在全局快捷键回调内部被调用，而是通过 spawn 延迟执行。
//      这避免了在回调还在执行时修改插件的内部注册表（竞态问题）。
//
//   2. RESTORING 标志在整个流程中为 true，防止模拟按键被自己的
//      全局快捷键二次拦截。
//
//   3. 流程：
//      注销快捷键 → 等待 10ms → 模拟原按键 → 等待 50ms → 重新注册快捷键
//
//   4. 重新注册时传入全新的闭包（等同于原来的 do_register 逻辑），
//      但因为不是在旧闭包内部递归调用，所以不会无限叠加。

/// 安全执行「注销 → 模拟 → 重注册」流程
///
/// 此函数应始终通过 tauri::async_runtime::spawn 在事件循环的下一个 tick 中调用，
/// 绝不能在全局快捷键的回调闭包内部直接调用。
fn do_restore_and_reregister(
    app_handle: AppHandle,
    shortcut_str: &'static str,
    action: Arc<MouseAction>,
    restore: Arc<RestoreAction>,
    last: Arc<Mutex<Instant>>,
) {
    // 步骤 1：注销快捷键
    if let Ok(sc) = shortcut_str.parse::<Shortcut>() {
        let _ = app_handle.global_shortcut().unregister(sc);
    }

    // 步骤 2：等待 OS 层面注销生效（10ms 经验值）
    std::thread::sleep(Duration::from_millis(10));

    // 步骤 3：设置 RESTORING 标志，防止二次拦截
    RESTORING.store(true, Ordering::SeqCst);

    // 步骤 4：用 enigo 模拟原按键（此时快捷键已注销 + RESTORING=true，不会被拦截）
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        match &*restore {
            RestoreAction::TypeText(text) => {
                let _ = enigo.text(text);
            }
            RestoreAction::SimulateKey(key) => {
                // Direction::Click = 按下 + 释放，模拟一次完整按键
                let _ = enigo.key(*key, Direction::Click);
            }
        }
    }

    // 步骤 5：等待目标应用处理完模拟按键（50ms 经验值）
    std::thread::sleep(Duration::from_millis(50));

    // 步骤 6：清除 RESTORING 标志
    RESTORING.store(false, Ordering::SeqCst);

    // 步骤 7：重新注册快捷键（传入全新的、非递归的闭包）
    register_single_shortcut(
        &app_handle,
        shortcut_str,
        action,
        restore,
        last,
    );
}

// =============================================================================
// 快捷键注册（内部函数）
// =============================================================================

/// 为单个快捷键注册回调
///
/// 回调逻辑：
///   - 先检查 RESTORING 标志（正在恢复中则放行，不拦截）
///   - 然后防抖 300ms
///   - 覆盖模式 ON  → 执行自定义鼠标动作
///   - 覆盖模式 OFF → spawn 异步任务执行 restore 流程（不阻塞回调）
fn register_single_shortcut(
    app_handle: &AppHandle,
    shortcut_str: &'static str,
    action: Arc<MouseAction>,
    restore: Arc<RestoreAction>,
    last: Arc<Mutex<Instant>>,
) {
    let sc: Shortcut = match shortcut_str.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[Overlay] 快捷键解析失败: {} - {}",
                shortcut_str, e
            );
            return;
        }
    };

    // clone 出闭包需要捕获的所有变量
    let app_handle_for_closure = app_handle.clone();
    let last = last.clone();
    let action = action.clone();
    let restore = restore.clone();

    let result = app_handle.global_shortcut().on_shortcut(
        sc,
        move |_app, _sc, _event| {
            
            // ★ 防重入：如果正在恢复（模拟原按键），直接放行
            if RESTORING.load(Ordering::SeqCst) {
                return;
            }

            // ★ 防抖：300ms 内重复触发直接忽略
            let now = Instant::now();
            let mut lt = last.lock().unwrap();
            if now.duration_since(*lt) < Duration::from_millis(500) {
                return;
            }
            *lt = now;

            if OVERLAY_ACTIVE.load(Ordering::Relaxed) {
                // 覆盖模式 ON：执行自定义鼠标动作
                execute_mouse_action(&action);
            } else {
                // 覆盖模式 OFF：将"注销→模拟→重注册"延迟到事件循环的下一个 tick
                let ah = app_handle_for_closure.clone();
                let shortcut_str = shortcut_str;
                let action = action.clone();
                let restore = restore.clone();
                let last = last.clone();

                tauri::async_runtime::spawn(async move {
                    do_restore_and_reregister(
                        ah,
                        shortcut_str,
                        action,
                        restore,
                        last,
                    );
                });
            }
        },
    );

    if let Err(e) = result {
        eprintln!(
            "[Overlay] 注册快捷键失败: {} - {:?}",
            shortcut_str, e
        );
    }
}

// =============================================================================
// 快捷键注册入口（公开 API）
// =============================================================================

/// 注册一个可覆盖的鼠标快捷键
///
/// # 参数
///   - `app`:      Tauri App 实例
///   - `shortcut`: 快捷键字符串，如 "Space", "F1"
///   - `action`:   覆盖模式下执行的鼠标动作
///   - `restore`:  覆盖模式关闭时模拟的原按键动作
///
/// # 行为
///   - 覆盖模式 ON：  执行 action 定义的鼠标动作
///   - 覆盖模式 OFF： 通过 spawn 异步执行「注销→模拟→重注册」，
///                    将按键透传给目标应用程序
fn reg_mouse(
    app: &tauri::App,
    shortcut: &'static str,
    action: MouseAction,
    restore: RestoreAction,
) {
    let app_handle = app.handle().clone();

    // 用 Arc 包装，让多个闭包共享所有权
    let action = Arc::new(action);
    let restore = Arc::new(restore);

    // 防抖时间记录（初始化为 1 秒前，确保首次触发不抖动）
    let last = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));

    register_single_shortcut(
        &app_handle,
        shortcut,
        action,
        restore,
        last,
    );
}

// =============================================================================
// 初始化入口
// =============================================================================

/// 初始化覆盖层模块
///
/// 在 Tauri 应用的 setup 阶段调用，完成：
///   1. 注册覆盖模式切换键（Shift+L）
///   2. 获取真实屏幕分辨率
///   3. 注册所有可覆盖的快捷键
pub fn init(app: &tauri::App) {
    // ──────────────────────────────────────────────────────────
    // 1. 注册覆盖模式切换键：Shift+L
    // ──────────────────────────────────────────────────────────
    let last_shift_l = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));

    let sc: Shortcut = "Shift+l".parse().expect("无效的快捷键字符串: Shift+l");
    let _ = app.global_shortcut().on_shortcut(sc, {
        let last = last_shift_l.clone();
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut lt = last.lock().unwrap();
            if now.duration_since(*lt) < Duration::from_millis(300) {
                return;
            }
            *lt = now;

            let new = !OVERLAY_ACTIVE.load(Ordering::Relaxed);
            OVERLAY_ACTIVE.store(new, Ordering::Relaxed);
            println!(
                "[Overlay] 覆盖模式 {}",
                if new { "ON" } else { "OFF" }
            );
        }
    });

    // ──────────────────────────────────────────────────────────
    // 2. 获取真实屏幕尺寸
    // ──────────────────────────────────────────────────────────
    let (sw, sh) = Enigo::new(&Settings::default())
        .expect("无法创建 Enigo 实例——请确保 enigo 支持当前平台")
        .main_display()
        .unwrap_or((1920, 1080));

    SCREEN_WIDTH.store(sw as i32, Ordering::Relaxed);
    SCREEN_HEIGHT.store(sh as i32, Ordering::Relaxed);

    println!("[Overlay] 屏幕分辨率: {}x{}", sw, sh);

    // ──────────────────────────────────────────────────────────
    // 3. 注册所有可覆盖的快捷键
    // ──────────────────────────────────────────────────────────
    reg_mouse(
        app,
        "Space",
        MouseAction::MoveToClick(sw / 2, sh / 2),
        RestoreAction::SimulateKey(Key::Space),
    );

    reg_mouse(
        app,
        "F1",
        MouseAction::ToggleLeftRightCenter,
        RestoreAction::SimulateKey(Key::F1),
    );
}
