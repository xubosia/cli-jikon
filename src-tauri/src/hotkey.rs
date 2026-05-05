use tauri::Manager;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

// 全局快捷键开关状态
static HOTKEY_ENABLED: AtomicBool = AtomicBool::new(true);
// 已注册的自定义快捷键字符串列表
static CUSTOM_HOTKEYS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// 注册一个打开 URL 的快捷键（AppHandle 版本）
fn reg_url(app_handle: &AppHandle, shortcut: &str, url: &'static str) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    match app_handle.global_shortcut().on_shortcut(shortcut, {
        let app_handle = app_handle.clone();
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
            let _ = app_handle.opener().open_url(url, None::<&str>);
        }
    }) {
        Ok(_) => {
            CUSTOM_HOTKEYS.lock().unwrap().push(shortcut.to_string());
        }
        Err(e) => {
            eprintln!("注册 {} 失败: {}", shortcut, e);
        }
    }
}

/// 注册一个启动程序的快捷键（AppHandle 版本）
fn reg_program(app_handle: &AppHandle, shortcut: &str, program: &'static str, args: Vec<&'static str>) {
    let last = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    match app_handle.global_shortcut().on_shortcut(shortcut, {
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
    }) {
        Ok(_) => {
            CUSTOM_HOTKEYS.lock().unwrap().push(shortcut.to_string());
        }
        Err(e) => {
            eprintln!("注册 {} 失败: {}", shortcut, e);
        }
    }
}

/// 注册所有自定义快捷键（可反复调用，内部会自动清理并重建）
fn register_custom_hotkeys(app_handle: &AppHandle) {
    // 先注销所有已存在的记录（防止重复注册）
    let mut hotkeys = CUSTOM_HOTKEYS.lock().unwrap();
    for h in hotkeys.iter() {
        if let Err(e) = app_handle.global_shortcut().unregister(h.as_str()) {
            eprintln!("注销旧快捷键 {} 失败: {}", h, e);
        }
    }
    hotkeys.clear();
    drop(hotkeys);

    // 注册实际快捷键
    reg_program(app_handle, "Shift+1",
        r"C:\Users\Administrator\AppData\Local\Doubao\Application\app\Doubao_browser_proxy.exe",
        vec!["https://www.douyin.com/?recommend=1"]
    );
     reg_program(app_handle, "Shift+4",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        vec!["https://github.com/"]
    );
    reg_url(app_handle, "Shift+2", "https://www.bilibili.com/index.php");
    reg_url(app_handle, "Shift+3", "https://chat.deepseek.com/");
    // 想增加更多快捷键，在这里继续添加 reg_url 或 reg_program 即可
}

pub fn register_hotkeys(app: &tauri::App) {
    let app_handle = app.handle().clone();

    // 首次注册自定义快捷键
    register_custom_hotkeys(&app_handle);

    // 注册 Shift+M 开关（带防抖，始终有效）
    let last_shift_m = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let app_handle_for_switch = app_handle.clone();
    let _ = app.global_shortcut().on_shortcut("Shift+m", {
        let last = last_shift_m.clone();
        let app_handle = app_handle_for_switch.clone();
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
                register_custom_hotkeys(&app_handle);
                println!("[Hotkey] 全局快捷键已启用");
            } else {
                let mut hotkeys = CUSTOM_HOTKEYS.lock().unwrap();
                for h in hotkeys.iter() {
                    if let Err(e) = app_handle.global_shortcut().unregister(h.as_str()) {
                        eprintln!("注销 {} 失败: {}", h, e);
                    }
                }
                hotkeys.clear();
                println!("[Hotkey] 全局快捷键已禁用");
            }
        }
    });
}

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