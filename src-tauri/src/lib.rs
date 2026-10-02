mod ai;
mod commands;
mod error;
mod jsonspan;
mod model;
mod rpgm;
mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ai::runner::AiState::default())
        .invoke_handler(tauri::generate_handler![
            commands::project::open_project,
            commands::project::save_project,
            commands::export::export_project,
            commands::ai::ai_translate,
            commands::ai::ai_cancel,
            ai::settings::get_ai_settings,
            ai::settings::save_ai_settings,
            ai::settings::default_system_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
