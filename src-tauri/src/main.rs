mod commands;
mod hotkey;
mod models;

use tauri::Manager; // <--- 这一行是必须的，否则 get_webview_window 找不到
use tauri_plugin_global_shortcut::GlobalShortcutExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let window = match app.get_webview_window("main") {
                Some(w) => w.clone(),
                None => {
                    eprintln!("无法获取主窗口");
                    return Ok(());
                }
            };

            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.set_always_on_top(true);

            // 原有的显示/隐藏快捷键
            if let Err(e) = app.global_shortcut().on_shortcut(
                "Ctrl+Shift+space",
                move |_app, _shortcut, _event| {
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

            // 新增的批量快捷键（从 hotkey 模块）
            hotkey::register_hotkeys(app);
            // 保活线程
            hotkey::start_keep_alive(app.handle().clone());

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
        ])
        .run(tauri::generate_context!())
        .expect("error");
}