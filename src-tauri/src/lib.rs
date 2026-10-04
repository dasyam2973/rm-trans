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
            commands::project::extract_entries,
            commands::project::save_project,
            commands::export::export_project,
            commands::assets::scan_assets,
            commands::assets::derive_asset_key,
            commands::assets::read_asset,
            commands::assets::export_assets,
            commands::assets::save_asset,
            commands::assets::set_asset_replacement,
            commands::assets::remove_asset_replacement,
            commands::assets::read_asset_replacement,
            commands::ai::ai_translate,
            commands::ai::ai_cancel,
            ai::settings::get_ai_settings,
            ai::settings::save_ai_settings,
            ai::settings::default_system_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
