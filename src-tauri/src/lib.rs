// src-tauri/src/lib.rs
mod commands;
mod config;
mod models;
mod utils;

use crate::config::GLOBAL_APP_HANDLE;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(tokio::sync::Mutex::new(()))
        .setup(|app| {
            // 获取 handle
            let handle = app.handle();
            // 初始化全局变量 (注意：Tauri v1/v2 handle 克隆成本很低)
            // config::GLOBAL_APP_HANDLE.set(handle).unwrap();
            GLOBAL_APP_HANDLE.set(handle.clone()).unwrap();
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::config_save,
            commands::config_get,
            commands::config_delete,
            commands::config_test,
            commands::file_list,
            commands::file_download,
            commands::file_delete,
            commands::file_get_preview_url,
            commands::file_upload,
            commands::file_download_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
