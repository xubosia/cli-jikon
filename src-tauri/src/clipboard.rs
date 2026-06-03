// src-tauri/src/clipboard.rs
use tauri::Emitter;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use enigo::{Enigo, Keyboard, Key, Direction, Settings};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::thread;
use tauri::Manager;

/// 向前端传递的剪贴板动作
#[derive(Clone, serde::Serialize)]
pub struct ClipboardAction {
    pub text: String,
    pub mode: String, // "browser" 或 "command"
}

/// 安全地模拟 Ctrl+C：释放 Shift → Ctrl+C → 恢复 Shift
fn simulate_copy() {
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        // 先释放 Shift（避免触发 Ctrl+Shift+C 自身）
        let _ = enigo.key(Key::Shift, Direction::Release);
        // 模拟 Ctrl+C
        let _ = enigo.key(Key::Control, Direction::Press);
        let _ = enigo.key(Key::C, Direction::Click);
        let _ = enigo.key(Key::Control, Direction::Release);
        // 恢复 Shift 按下状态
        let _ = enigo.key(Key::Shift, Direction::Press);
    }
}

/// 读取系统剪贴板文本（跨平台）
fn read_clipboard() -> String {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.get_text())
        .unwrap_or_default()
}

/// 注册 Ctrl+Shift+C 和 Ctrl+Alt+C
pub fn register_clipboard_shortcuts(app: &tauri::App) {
    let handle = app.handle().clone();

    // Ctrl+Shift+C → 浏览器搜索（带防抖 + 自动复制）
    let last1 = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut(
        "Ctrl+Shift+x",
        {
            let last = last1.clone();
            move |_app, _shortcut, _event| {
                let now = Instant::now();
                let mut last_time = last.lock().unwrap();
                if now.duration_since(*last_time) < Duration::from_millis(500) {
                    return;
                }
                *last_time = now;

                // 自动复制当前选中的文本
                simulate_copy();
                // 等待剪贴板更新，必须 ≥ 300ms
                thread::sleep(Duration::from_millis(300));

                let text = read_clipboard();
                if !text.is_empty() {
                    let _ = handle.emit("clipboard-action", ClipboardAction {
                        text,
                        mode: "browser".to_string(),
                    });
                }
            }
        },
    );

    // Ctrl+Alt+C → 命令行执行（带防抖 + 自动复制）
    let handle2 = app.handle().clone();
    let last2 = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let _ = app.global_shortcut().on_shortcut(
        "Ctrl+Alt+x",
        {
            let last = last2.clone();
            move |_app, _shortcut, _event| {
                let now = Instant::now();
                let mut last_time = last.lock().unwrap();
                if now.duration_since(*last_time) < Duration::from_millis(500) {
                    return;
                }
                *last_time = now;

                simulate_copy();
                thread::sleep(Duration::from_millis(300));

                let text = read_clipboard();
                if !text.is_empty() {
                    let _ = handle2.emit("clipboard-action", ClipboardAction {
                        text,
                        mode: "command".to_string(),
                    });
                }
            }
        },
    );
}
/// 模拟键盘输入文本（通过剪贴板粘贴）并自动按回车
#[tauri::command]
pub fn simulate_input_and_hide(text: &str, app: tauri::AppHandle) -> Result<(), String> {
    use enigo::{Enigo, Keyboard, Key, Direction, Settings};
    use std::thread::sleep;
    use std::time::Duration;

    // 1. 隐藏窗口（直接使用 app handle，和全局快捷键一样的方式）
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // 2. 短暂等待窗口完全失去焦点
    sleep(Duration::from_millis(200));

    // 3. 复制文本到剪贴板
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("剪贴板打开失败: {}", e))?;
    clipboard.set_text(text)
        .map_err(|e| format!("剪贴板设置失败: {}", e))?;

    // 4. 释放干扰键并模拟 Ctrl+V
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Enigo 创建失败: {}", e))?;
    let _ = enigo.key(Key::Shift, Direction::Release);
    let _ = enigo.key(Key::Control, Direction::Release);
    let _ = enigo.key(Key::Alt, Direction::Release);
    sleep(Duration::from_millis(200));//释放干扰键后短暂等待，确保系统状态稳定
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::V, Direction::Click);
    let _ = enigo.key(Key::Control, Direction::Release);

    // 5. 模拟回车
    sleep(Duration::from_millis(50));
    let _ = enigo.key(Key::Return, Direction::Click);

    Ok(())
}