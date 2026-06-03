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
// 中文标点模式开关（true = 中文标点，false = 英文标点）
static CHINESE_PUNCTUATION: AtomicBool = AtomicBool::new(false);

/// 根据快捷键字符串模拟输出对应的字符
fn simulate_char(shortcut: &str) {
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let chinese_mode = CHINESE_PUNCTUATION.load(Ordering::Relaxed);
        
        let text = match shortcut {
            // ========== 英文标点 ==========
            "Shift+1" if !chinese_mode => "!",
            "Shift+2" if !chinese_mode => "@",
            "Shift+3" if !chinese_mode => "#",
            "Shift+4" if !chinese_mode => "$",
            "Shift+5" if !chinese_mode => "%",
            "Shift+6" if !chinese_mode => "^",
            "Shift+7" if !chinese_mode => "&",
            "Shift+8" if !chinese_mode => "*",
            "Shift+9" if !chinese_mode => "(",
            "Shift+0" if !chinese_mode => ")",
            
            // ========== 中文标点 ==========
            "Shift+1" => "！",
            "Shift+2" => "＠",   // 全角 @（也可改为 · 或自定义）
            "Shift+3" => "＃",
            "Shift+4" => "￥",   // 人民币符号
            "Shift+5" => "％",
            "Shift+6" => "……",  // 省略号
            "Shift+7" => "＆",
            "Shift+8" => "＊",
            "Shift+9" => "（",
            "Shift+0" => "）",
            
            _ => return,
        };
        let _ = enigo.text(text);
    }
}

/// 注册一个打开 URL 的快捷键（带防抖 + 开关控制 + 禁用时模拟输出）
fn reg_url(app_handle: &AppHandle, shortcut: &'static str, url: &'static str) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app_handle.global_shortcut().on_shortcut(shortcut, {
        let app_handle = app_handle.clone();
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;

            if !HOTKEY_ENABLED.load(Ordering::Relaxed) {
                simulate_char(shortcut);
                return;
            }
            let _ = app_handle.opener().open_url(url, None::<&str>);
        }
    });
}

fn reg_program(app_handle: &AppHandle, shortcut: &'static str, program: &'static str, args: Vec<&'static str>) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app_handle.global_shortcut().on_shortcut(shortcut, {
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;

            if !HOTKEY_ENABLED.load(Ordering::Relaxed) {
                simulate_char(shortcut);
                return;
            }
            let _ = std::process::Command::new(program).args(&args).spawn();
        }
    });
}

/// 注册所有自定义快捷键
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
    reg_program(&app_handle, "Shift+5",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        vec!["https://www.youtube.com/"]
    );

    reg_url(&app_handle, "Shift+2", "https://www.bilibili.com/index.php");
    reg_url(&app_handle, "Shift+3", "https://chat.deepseek.com/");

    // ==================== Shift+M：开/关快捷键功能 ====================
    let last_shift_m = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut("Shift+M", {
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

    // ==================== Shift+N：切换中/英文标点模式 ====================
    let last_shift_n = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut("Shift+N", {
        let last = last_shift_n.clone();
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;
            let was_chinese = CHINESE_PUNCTUATION.fetch_xor(true, Ordering::Relaxed);
            let now_chinese = !was_chinese;
            if now_chinese {
                println!("[Hotkey] 标点模式：中文全角");
            } else {
                println!("[Hotkey] 标点模式：英文半角");
            }
        }
    });
}

pub fn start_keep_alive(app_handle: AppHandle) {
    thread::spawn(move || {
        let interval = Duration::from_secs(30);
        loop {
            thread::sleep(interval);
            if let Some(w) = app_handle.get_webview_window("main") {
               let _ = w.set_focus();
            }
        }
    });
}