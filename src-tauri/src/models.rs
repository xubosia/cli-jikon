use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedItem {
    pub name: String,
    pub path: String,
    // #[serde(rename = "isDir")]// 这是一个 serde 库特有的属性...
    pub is_dir: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedData {
    pub files: Vec<SavedItem>,
    pub folders: Vec<SavedItem>,
    pub apps: Vec<SavedItem>,
    pub input_history: Vec<String>,
    pub command_history: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchEngine {
    pub name: String,
    pub url_template: String,
}

#[derive(serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}