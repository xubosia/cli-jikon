// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod hotkey;
mod models;
mod overlay;
mod clipboard;
mod python_runner;   // ← 新增

use tauri::Manager; // <--- 这一行是必须的，否则 get_webview_window 找不到
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let window = match app.get_webview_window("main") //"main" 是窗口 ID（来自 tauri.conf.json）
            {
                Some(w) => w.clone(),//这是match模式匹配，这个w是个代称什么都行，总之匹配的话，match会将值传给它，总要有个东西来接收不是吗？
        //   Rust 的 match | 语言     | 写法          |
                        // | ------ | ----------- |
                        // | JS     | if + 解构     |
                        // | Python | if + 赋值     |
                        // | Rust   | 一步完成（match） |

                None => {
                    eprintln!("无法获取主窗口");
                    return Ok(());
                }
            };
            

            let _ = window.show();
            let _ = window.set_focus();
  
            let _ = window.set_always_on_top(true);

//{}:限制变量作用域

// 避免 last_toggle、window 泄漏到外面。
{
    let last_toggle = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let window = window.clone(); // 克隆一份 window，防止所有权问题，这是一个令人极其头疼的问题，好在有些东西实在不大。clone，clone就行了。

    // 只关心错误情况，正确什么都不做。这是效率和原理的哲学；
    // 错误不是异常（exception）是新的值。
    if let Err(e) = app.global_shortcut().on_shortcut(
        "Ctrl+Shift+space",
        move |_app, _shortcut, _event| {
            let now = Instant::now();
            let mut last = last_toggle.lock().unwrap();
            if now.duration_since(*last) < Duration::from_millis(300) {
                return; // 300ms 内的重复触发忽略
            }
            *last = now;

            let w = window.clone();
            if w.is_visible().unwrap_or(false) {
                let _ = w.set_always_on_top(false);
                let _ = w.hide();
            } else {
                let _ = w.show();
                let _ = w.set_always_on_top(true);
                let _ = w.set_focus();
            }
        },
    ) {
        eprintln!("全局快捷键注册失败: {}", e);
    }
}


            // 新增的批量快捷键（从 hotkey 模块）
            hotkey::register_hotkeys(app);
            // 保活线程
            hotkey::start_keep_alive(app.handle().clone());

            overlay::init(app);
            clipboard::register_clipboard_shortcuts(app);
            python_runner::setup_python_backend(app);   // 启动 Python 后端

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_path,
            commands::read_dir,
            commands::load_data,
            commands::save_data,
            commands::open_with,
            commands::execute_command,
            commands::get_search_engines,
            commands::toggle_always_on_top,
            commands::get_always_on_top,
            commands::focus_main_window,
            clipboard::simulate_input_and_hide, 
        ])
        .run(tauri::generate_context!())
        .expect("error");
}