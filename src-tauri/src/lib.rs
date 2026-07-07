// src-tauri/src/lib.rs
mod commands;
mod config;
mod models;
mod utils;

use crate::config::GLOBAL_APP_HANDLE;
use tauri::image::Image;
use tauri::menu::{IconMenuItem, Menu, PredefinedMenuItem, Submenu};
use tauri::Emitter;

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

            let settings_icon = Image::from_bytes(include_bytes!("../icons/setting.png")).ok();
            let update_icon = Image::from_bytes(include_bytes!("../icons/update_menu.png")).ok();
            let settings_item = IconMenuItem::with_id(
                app,
                "open_settings",
                "系统设置",
                true,
                settings_icon,
                None::<&str>,
            )?;
            let check_update_item = IconMenuItem::with_id(
                app,
                "check_update",
                "检查更新",
                true,
                update_icon,
                None::<&str>,
            )?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = PredefinedMenuItem::quit(app, Some("退出"))?;
            let app_menu = Submenu::with_items(
                app,
                "S3 Client",
                true,
                &[&settings_item, &check_update_item, &separator, &quit_item],
            )?;
            let menu = Menu::with_items(app, &[&app_menu])?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "open_settings" {
                let _ = app.emit("open_settings", ());
            } else if event.id().as_ref() == "check_update" {
                let _ = app.emit("check_update", ());
            }
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::app_update_check,
            commands::app_update_skip,
            commands::app_update_install,
            commands::config_save,
            commands::config_get,
            commands::config_sort_save,
            commands::config_delete,
            commands::config_test,
            commands::settings_get,
            commands::settings_save,
            commands::bucket_list,
            commands::bucket_probe_permissions,
            commands::file_list,
            commands::file_download,
            commands::file_delete,
            commands::file_get_preview_url,
            commands::file_create_presigned_url,
            commands::file_create_directory,
            commands::file_copy,
            commands::file_move,
            commands::file_upload,
            commands::file_download_path,
            commands::file_export_parquet_xlsx,
            commands::file_transfer_cancel,
            commands::file_transfer_retry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
