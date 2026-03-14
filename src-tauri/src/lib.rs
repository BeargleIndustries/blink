mod commands;
mod state;
mod error;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let backend = sd_wrapper::gpu::compiled_backend();
            log::info!("GPU backend: {}", backend);
            if backend == sd_wrapper::gpu::GpuBackend::Cpu {
                log::warn!("No GPU acceleration detected. Image generation will be significantly slower.");
                log::warn!("For faster generation, install the CUDA Toolkit (NVIDIA) or use a Mac with Metal support.");
            }

            let app_state = state::AppState::new(app.handle().clone())?;
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::generation::generate_image,
            commands::generation::cancel_generation,
            commands::models::get_models,
            commands::models::get_downloaded_models,
            commands::models::download_model,
            commands::models::delete_model,
            commands::models::set_active_model,
            commands::models::get_download_progress,
            commands::models::import_custom_model,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_hf_token,
            commands::settings::set_hf_token,
            commands::settings::get_perf_settings,
            commands::settings::save_perf_settings,
            commands::system::get_system_info,
            commands::system::get_app_version,
            commands::system::get_licenses,
            commands::gallery::get_gallery,
            commands::gallery::delete_gallery_item,
            commands::gallery::export_image,
            commands::gallery::save_to_gallery,
            commands::gallery::load_gallery_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running blink");
}
