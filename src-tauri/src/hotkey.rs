// src-tauri/src/hotkey.rs
use tauri::Manager;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

// 全局快捷键开关（默认开启）
static HOTKEY_ENABLED: AtomicBool = AtomicBool::new(true);

/// 注册一个打开 URL 的快捷键（带防抖，且受全局开关控制）
fn reg_url(app: &tauri::App, shortcut: &str, url: &'static str) {
    let app_handle = app.handle().clone();
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut(shortcut, {
        let app_handle = app_handle.clone();
        move |_app, _shortcut, _event| {
            // 检查快捷键是否被禁用
            if !HOTKEY_ENABLED.load(Ordering::Relaxed) {
                return;
            }
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;
            let _ = app_handle.opener().open_url(url, None::<&str>);
        }
    });
}

/// 注册一个启动程序的快捷键（带防抖，且受全局开关控制）
fn reg_program(app: &tauri::App, shortcut: &str, program: &'static str, args: Vec<&'static str>) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut(shortcut, {
        move |_app, _shortcut, _event| {
            if !HOTKEY_ENABLED.load(Ordering::Relaxed) {
                return;
            }
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return;
            }
            *last_time = now;
            let _ = std::process::Command::new(program).args(&args).spawn();
        }
    });
}

/// 注册所有自定义全局快捷键，并增加 Shift+M 开关（带防抖）
pub fn register_hotkeys(app: &tauri::App) {
    // ---------- 自定义快捷键 ----------
    reg_program(app, "Shift+1",
        r"C:\Users\Administrator\AppData\Local\Doubao\Application\app\Doubao_browser_proxy.exe",
        vec!["https://www.douyin.com/?recommend=1"]
    );
    reg_url(app, "Shift+2", "https://www.bilibili.com/index.php");
    reg_url(app, "Shift+3", "https://chat.deepseek.com/");

    // ---------- Shift+M 开关（带防抖） ----------
    let last_shift_m = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut("Shift+m", {
        let last = last_shift_m.clone();
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last_time = last.lock().unwrap();
            if now.duration_since(*last_time) < Duration::from_millis(300) {
                return; // 防抖：忽略 300ms 内的重复触发
            }
            *last_time = now;
            // 切换全局快捷键启用状态
            let was_enabled = HOTKEY_ENABLED.fetch_xor(true, Ordering::Relaxed);
            let now_enabled = !was_enabled;
            if now_enabled {
                println!("[Hotkey] 全局快捷键已启用");
            } else {
                println!("[Hotkey] 全局快捷键已禁用");
            }
        }
    });
}

/// 后台保活线程（每分钟轻量操作一次，防系统挂起）
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