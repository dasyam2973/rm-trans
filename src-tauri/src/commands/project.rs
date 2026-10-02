use std::path::PathBuf;

use serde::Serialize;

use crate::error::Result;
use crate::model::{Engine, Entry};
use crate::rpgm::extract::Extracted;
use crate::rpgm::{detect, extract};
use crate::store::{self, ProjectFile};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedProject {
    pub root: String,
    pub data_dir: String,
    pub engine: Engine,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
    /// 이전에 저장해 둔 작업 상태
    pub saved: Option<ProjectFile>,
}

#[tauri::command]
pub async fn open_project(path: String) -> Result<OpenedProject> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(&path);
        let layout = detect::detect(&root)?;
        let saved = store::load(&root)?;
        let include_plugins = saved.as_ref().is_some_and(|s| s.options.include_plugins);
        let Extracted { entries, warnings } = extract::extract_all(&layout, include_plugins)?;
        Ok(OpenedProject { root: path, data_dir: layout.data_rel, engine: layout.engine, entries, warnings, saved })
    })
    .await
    .expect("open_project 작업 패닉")
}

/// 추출 옵션을 바꿨을 때 아이템 목록만 다시 만든다.
#[tauri::command]
pub async fn extract_entries(root: String, include_plugins: bool) -> Result<Extracted> {
    tauri::async_runtime::spawn_blocking(move || {
        let layout = detect::detect(&PathBuf::from(root))?;
        extract::extract_all(&layout, include_plugins)
    })
    .await
    .expect("extract_entries 작업 패닉")
}

#[tauri::command]
pub async fn save_project(root: String, project: ProjectFile) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || store::save(&PathBuf::from(root), &project))
        .await
        .expect("save_project 작업 패닉")
}
