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
pub async fn open_path(_app: tauri::AppHandle, path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if std::path::Path::new(&path).is_dir() {
            std::process::Command::new("explorer")
                .arg(&path)
                .creation_flags(0x00000008)
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            // 使用 shell 的 "open" 或者 cmd /c start "" 的方式
            // 推荐使用 explorer 或者直接调用关联程序？最简单可靠：
            std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", &path])
                .creation_flags(0x00000008)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 用 opener 插件打开（稳定）
        app.opener()
            .open_path(path, None::<&str>)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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
        // ★ 关键：使用 creation_flags 让子进程脱离父进程
        use std::os::windows::process::CommandExt;
        Command::new(&app_path)
            .arg(&folder_path)
            .creation_flags(0x00000008)   // DETACHED_PROCESS，禁用继承父进程控制台
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]

pub async fn execute_command(cmd: String) -> Result<String, String> {
    // 跨平台：Windows 下启动新窗口，其他平台保持原样（可自行扩展）-cmd /k 启动新窗口并执行命令,c执行完关闭
    #[cfg(target_os = "windows")]
    {
        // 使用 start cmd /c 打开新窗口执行命令，执行完自动关闭
        std::process::Command::new("cmd")
            .args(["/C", &format!("start cmd /k {}", cmd)])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok("已在单独窗口中运行".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 暂不处理（可添加对应终端启动命令）
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.success() {
            Ok(stdout)
        } else {
            Err(format!("{}\n{}", stdout, stderr))
        }
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