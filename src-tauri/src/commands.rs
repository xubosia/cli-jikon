use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use std::fs;
use std::process::Command;
use crate::models::*;

#[tauri::command]
pub fn get_search_engines() -> Vec<SearchEngine> {
    vec![
        SearchEngine {
            name: "Bing".into(),
            url_template: "https://www.bing.com/search?q={q}".into(),
        },
        SearchEngine {
            name: "百度".into(),
            url_template: "https://www.baidu.com/s?wd={q}".into(),
        },
        SearchEngine {
            name: "Bilibili".into(),
            url_template: "https://search.bilibili.com/all?keyword={q}".into(),
        },
    ]
}

#[tauri::command]
pub async fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let entries = fs::read_dir(&path).map_err(|e| e.to_string())?;
    let mut result = vec![];
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        result.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: file_type.is_dir(),
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn load_data(app: tauri::AppHandle) -> Result<SavedData, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let file_path = data_dir.join("app_data.json");
    if file_path.exists() {
        let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        let data: SavedData = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(data)
    } else {
        Ok(SavedData {
            files: vec![],
            folders: vec![],
            apps: vec![],
            input_history: vec![],
            command_history: vec![],
        })
    }
}

#[tauri::command]
pub async fn save_data(app: tauri::AppHandle, data: SavedData) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let file_path = data_dir.join("app_data.json");
    let content = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(&file_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_with(folder_path: String, app_path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-a", &app_path, &folder_path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        Command::new(&app_path)
            .arg(&folder_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn execute_command(cmd: String) -> Result<String, String> {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", &cmd])
            .output()
    } else {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
    }
    .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!("{}\n{}", stdout, stderr))
    }
}

#[tauri::command]
pub async fn toggle_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("main").ok_or("窗口不存在")?;
    let new_state = !window.is_always_on_top().unwrap_or(false);
    window.set_always_on_top(new_state).map_err(|e| e.to_string())?;
    Ok(new_state)
}

#[tauri::command]
pub async fn get_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("main").ok_or("窗口不存在")?;
    window.is_always_on_top().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn focus_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("窗口不存在")?;
    window.set_focus().map_err(|e| e.to_string())
}