use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::os::windows::process::CommandExt;
use tauri::{App, Manager};
use tauri::path::BaseDirectory;

/// 保存子进程句柄，方便退出时清理
pub struct PythonBackend(pub Arc<Mutex<Option<Child>>>);

impl Drop for PythonBackend {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(ref mut child) = *guard {
                let _ = child.kill();
                let _ = child.wait();
                println!("Python 后端进程已终止");
            }
        }
    }
}

/// 启动 Python Flask 后端
pub fn setup_python_backend(app: &App) {
    // 根据编译模式决定资源目录和脚本路径
    let (script_path, models_dir): (PathBuf, PathBuf) = if cfg!(debug_assertions) {
        // 开发模式：直接使用项目目录下的文件
        let current_dir = std::env::current_dir()
            .expect("无法获取当前目录");
        let script_path = current_dir
            .join("ai_server")
            .join("server.py");
        let models_dir = current_dir
            .join("ai_server")
            .join("models");
        (script_path, models_dir)
    } else {
        // 发布模式：使用 Tauri v2 资源目录 API
        let resource_path = app.path()
            .resolve("ai_server/server.py", BaseDirectory::Resource)
            .expect("无法解析 Python 脚本路径");
        let models_path = app.path()
            .resolve("ai_server/models", BaseDirectory::Resource)
            .expect("无法解析模型目录路径");
        (resource_path, models_path)
    };

    // 验证脚本路径存在
    if !script_path.exists() {
        eprintln!("Python 脚本不存在: {:?}", script_path);
        app.manage(PythonBackend(Arc::new(Mutex::new(None))));
        return;
    }

    let child = match Command::new("python")
        .arg(&script_path)
        .env("MODELS_DIR", &models_dir)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW - 隐藏命令行窗口
        .spawn()
    {
        Ok(child) => {
            println!("Python 后端已启动，PID: {}", child.id());
            Some(child)
        }
        Err(e) => {
            eprintln!("启动 Python 后端失败: {}. 请确认已安装 Python 及依赖。", e);
            None
        }
    };

    app.manage(PythonBackend(Arc::new(Mutex::new(child))));
}