use tauri::Manager;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use enigo::{Enigo, Keyboard, Settings};
// 全局快捷键开关状态
static HOTKEY_ENABLED: AtomicBool = AtomicBool::new(true);

/// 根据快捷键字符串模拟输出对应的字符
fn simulate_char(shortcut: &str) {
    let key_str = match shortcut {
        "Shift+1" => "!",
        "Shift+2" => "@",
        "Shift+3" => "#",
        "Shift+4" => "$",
        "Shift+5" => "%",
        "Shift+6" => "^",
        "Shift+7" => "&",
        "Shift+8" => "*",
        "Shift+9" => "(",
        "Shift+0" => ")",
        // 可在此继续扩展其他快捷键
        _ => return,
    };
    // 使用 enigo 输入文本
 if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.text(key_str);
    }
}

/// 注册一个打开 URL 的快捷键（带防抖 + 开关控制 + 禁用时模拟输出）
fn reg_url(app_handle: &AppHandle, shortcut: &'static str, url: &'static str) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app_handle.global_shortcut().on_shortcut(shortcut, {
        let app_handle = app_handle.clone();
        move |_app, _shortcut, _event| {
            // 防抖（最先执行）
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;

            // 根据开关状态分发
            if !HOTKEY_ENABLED.load(Ordering::Relaxed) {
                simulate_char(shortcut);   // 模拟原本的字符
                return;
            }
            // 正常执行
            let _ = app_handle.opener().open_url(url, None::<&str>);
        }
    });
}

fn reg_program(app_handle: &AppHandle, shortcut: &'static str, program: &'static str, args: Vec<&'static str>) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app_handle.global_shortcut().on_shortcut(shortcut, {
        move |_app, _shortcut, _event| {
            // 防抖
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;

            // 分发
            if !HOTKEY_ENABLED.load(Ordering::Relaxed) {
                simulate_char(shortcut);
                return;
            }
            let _ = std::process::Command::new(program).args(&args).spawn();
        }
    });
}
/// 注册所有自定义快捷键，并添加 Shift+M 开关
pub fn register_hotkeys(app: &tauri::App) {
    let app_handle = app.handle().clone();

    // 初始注册所有自定义快捷键
    reg_program(&app_handle, "Shift+1",
        r"C:\Users\Administrator\AppData\Local\Doubao\Application\app\Doubao_browser_proxy.exe",
        vec!["https://www.douyin.com/?recommend=1"]
    );
    reg_program(&app_handle, "Shift+4",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        vec!["https://github.com/"]
    );
    reg_url(&app_handle, "Shift+2", "https://www.bilibili.com/index.php");
    reg_url(&app_handle, "Shift+3", "https://chat.deepseek.com/");
    // 未来添加更多快捷键，直接在这里加一行 reg_url 或 reg_program 即可

    // 注册开关快捷键 Shift+M（永远生效，自带防抖）
    let last_shift_m = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut("Shift+m", {
        let last = last_shift_m.clone();
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;
            let was_enabled = HOTKEY_ENABLED.fetch_xor(true, Ordering::Relaxed);
            let now_enabled = !was_enabled;
            if now_enabled {
                println!("[Hotkey] 全局快捷键已启用");
            } else {
                println!("[Hotkey] 全局快捷键已禁用 (自动打出符号)");
            }
        }
    });
}

/// 后台保活线程（不变）
pub fn start_keep_alive(app_handle: AppHandle) {
    thread::spawn(move || {
        let interval = Duration::from_secs(60);
        loop {
            thread::sleep(interval);
            if let Some(w) = app_handle.get_webview_window("main") {
                let _ = w.title();
            }
        }
    });
}