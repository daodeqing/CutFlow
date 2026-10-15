mod cutflow;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "android")]
    let builder = builder.plugin(bdl_mobile_credentials::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(bdl_mobile_execution::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(bdl_mobile_media::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(bdl_mobile_storage::init());

    let builder = builder
        .setup(cutflow::install)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let active = cutflow::active(window.app_handle());
                eprintln!("CutFlow close requested: active tasks={active}");
                if active {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    api.prevent_close();
                    cutflow::quit(window.app_handle().clone());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            cutflow::cutflow_call,
            bdl_tauri::commands::parse_create_source,
            bdl_tauri::commands::parse_load_more,
            bdl_tauri::commands::parse_load_page,
            bdl_tauri::commands::parse_cancel,
            bdl_tauri::commands::parse_progress,
            bdl_tauri::commands::parse_load_all,
            bdl_tauri::commands::parse_close_source,
            bdl_tauri::commands::parse_refresh_source,
            bdl_tauri::commands::selection_create_tasks,
            bdl_tauri::commands::selection_estimate_size,
            bdl_tauri::commands::mobile_pick_export_directory,
            bdl_tauri::commands::mobile_read_clipboard_text,
            bdl_tauri::commands::mobile_save_image_to_gallery,
            bdl_tauri::commands::mobile_prepare_notifications,
            bdl_tauri::commands::queue_list,
            bdl_tauri::commands::queue_output_sizes,
            bdl_tauri::commands::queue_startup_recovery,
            bdl_tauri::commands::queue_dismiss_startup_recovery,
            bdl_tauri::commands::queue_logs,
            bdl_tauri::commands::queue_pause,
            bdl_tauri::commands::queue_resume,
            bdl_tauri::commands::queue_schedule,
            bdl_tauri::commands::queue_unschedule,
            bdl_tauri::commands::queue_set_speed_limit,
            bdl_tauri::commands::queue_cancel,
            bdl_tauri::commands::queue_retry,
            bdl_tauri::commands::queue_refresh_urls_and_retry,
            bdl_tauri::commands::queue_bulk_pause,
            bdl_tauri::commands::queue_bulk_cancel,
            bdl_tauri::commands::queue_bulk_resume,
            bdl_tauri::commands::queue_bulk_retry,
            bdl_tauri::commands::queue_bulk_refresh_urls_and_retry,
            bdl_tauri::commands::queue_bulk_remove,
            bdl_tauri::commands::queue_clear_completed,
            bdl_tauri::commands::queue_remove,
            bdl_tauri::commands::queue_delete_preview,
            bdl_tauri::commands::queue_delete_files,
            bdl_tauri::commands::maintenance_temp_preview,
            bdl_tauri::commands::queue_open_file,
            bdl_tauri::commands::queue_open_dir,
            bdl_tauri::commands::open_external_url,
            bdl_tauri::commands::settings_get,
            bdl_tauri::commands::settings_update,
            bdl_tauri::commands::environment_health,
            bdl_tauri::commands::environment_create_download_directory,
            bdl_tauri::commands::maintenance_cleanup_cache,
            bdl_tauri::commands::maintenance_cleanup_temp,
            bdl_tauri::commands::diagnostics_export,
            bdl_tauri::commands::diagnostics_environment,
            bdl_tauri::commands::account_get,
            bdl_tauri::commands::account_login_qr_start,
            bdl_tauri::commands::account_login_qr_poll,
            bdl_tauri::commands::account_import_cookie,
            bdl_tauri::commands::account_logout,
            bdl_tauri::commands::account_verify,
            bdl_tauri::commands::account_library_list,
        ])
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_process::init());

    builder
        .run(tauri::generate_context!())
        .expect("failed to run CutFlow app");
}
