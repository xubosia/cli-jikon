// src-tauri/src/overlay.rs
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use enigo::{Enigo, Mouse, Settings, Coordinate, Keyboard, Button, Direction, Key};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering}; // 新增 AtomicI32
use std::sync::Mutex;
use std::time::{Duration, Instant}; 

// 覆盖模式开关
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);//读写操作是原子性的 —— 多线程同时修改 / 读取时，不会出现数据竞争，保证线程安全。
// 一句话总结
// 创建了一个线程安全、初始值为假的布尔变量，专门用来在多线程之间安全地共享状态。
// 鼠标左右切换标志
static MOUSE_ON_LEFT: AtomicBool = AtomicBool::new(true);

// 全局屏幕尺寸（默认1920x1080，启动时自动更新为实际值）
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(1920);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(1080);

/// 自定义鼠标动作
pub enum MouseAction {
    MoveTo(i32, i32),
    Click,
    DoubleClick,
    MoveTo_Click(i32, i32),
    ToggleLeftRightCenter,
}

/// 当覆盖模式关闭时，对应按键应执行的恢复动作
pub enum RestoreAction {
    /// 模拟输入一段文本（如空格 " "）
    TypeText(&'static str),
    /// 模拟按下并释放某个键（功能键等）
    SimulateKey(Key),
}

fn execute_mouse_action(action: &MouseAction) {
    // 直接从全局变量读取屏幕尺寸，不再依赖 Enigo 的 main_display()
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
                let _ = enigo.button(Button::Left, Direction::Click);
                std::thread::sleep(Duration::from_millis(50));
                let _ = enigo.button(Button::Left, Direction::Click);
            }
            MouseAction::MoveTo_Click(x, y) => {
                let _ = enigo.move_mouse(*x, *y, Coordinate::Abs);
                let _ = enigo.button(Button::Left, Direction::Click);
            }
            MouseAction::ToggleLeftRightCenter => {
                let x = if MOUSE_ON_LEFT.load(Ordering::Relaxed) {
                    sw / 4
                } else {
                    sw * 3 / 4
                };
                let y = sh / 2;
                let _ = enigo.move_mouse(x, y, Coordinate::Abs);
                let _ = enigo.button(Button::Left, Direction::Click);
                MOUSE_ON_LEFT.fetch_xor(true, Ordering::Relaxed);//取反，是一切按键切换的核心基础：
            }
        }
    }
}

/// 一行注册鼠标快捷键（永久注册，覆盖模式下执行自定义动作，关闭时自动恢复）
//reg_mouse 需要 app: &tauri::App 参数，是因为 tauri-plugin-global-shortcut 的快捷键注册必须通过 Tauri 的 App 或 AppHandle 来进行。这是底层架构决定的。
fn reg_mouse(
    app: &tauri::App,
    shortcut: &'static str,
    action: MouseAction,
    restore: RestoreAction,
) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut(shortcut, {
        let last = last.clone();
        move |_app, _shortcut, _event| {
        if OVERLAY_ACTIVE.load(Ordering::Relaxed) {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;

            execute_mouse_action(&action);
            } else {
                // 非覆盖模式：执行恢复动作
                run_restore(&restore);
            }
        }
    });
}

/// 执行恢复动作（模拟按键或文本）
 pub fn run_restore(restore: &RestoreAction) {
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        match restore {
            RestoreAction::TypeText(text) => {
                let _ = enigo.text(text);
            }
            RestoreAction::SimulateKey(key) => {
                let _ = enigo.key(*key, Direction::Click);
            }
        }
    }
}

/// 初始化覆盖层
pub fn init(app: &tauri::App) {
    // Shift+L 开关（始终有效，带防抖）
    let last_shift_l = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut("Shift+l", {
        let last = last_shift_l.clone();
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;
           let new = !OVERLAY_ACTIVE.load(Ordering::Relaxed);
            OVERLAY_ACTIVE.store(new, Ordering::Relaxed);
            println!("[Overlay] 覆盖模式 {}", if new { "ON" } else { "OFF" });
        }
    });

    // 获取真实屏幕尺寸并存入全局变量（此后所有函数均可直接读取）
    let (sw, sh) = Enigo::new(&Settings::default())
        .expect("无法创建 Enigo 实例")
        .main_display()
        .unwrap_or((1920, 1080));
    SCREEN_WIDTH.store(sw as i32, Ordering::Relaxed);
    SCREEN_HEIGHT.store(sh as i32, Ordering::Relaxed);

    // ★ 添加新动作只需加一行 reg_mouse ★
    reg_mouse(
        app,"Space",
        MouseAction::ToggleLeftRightCenter,
        RestoreAction::TypeText(" "),
    );
    reg_mouse(
        app,"F1",
        MouseAction::MoveTo_Click(sw / 2, sh / 2),
        RestoreAction::SimulateKey(Key::F1),
    );
    // 更多示例：
    // reg_mouse(app, "F2", MouseAction::MoveTo(100, 200), RestoreAction::SimulateKey(Key::F2));
    // reg_mouse(app, "F3", MouseAction::DoubleClick, RestoreAction::SimulateKey(Key::F3));
}