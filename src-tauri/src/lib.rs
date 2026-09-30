pub mod commands;
pub mod db;
pub mod errors;
pub mod models;
pub mod services;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(commands::scan::FileWatcherState {
            inner: std::sync::Mutex::new(None),
        })
        .manage(commands::deduplication::DedupState::default())
        .manage(commands::migration::MigrationState::default())
        .manage(commands::ingestion::IngestionState::default())
        .setup(|app| {
            // Initialize database in the app data directory
            let app_data_dir = app.path().app_data_dir()?;
            let database = db::init_db(&app_data_dir).expect("Failed to initialize database");

            // Run auto-purge for expired trash on startup
            let _ = database.with_conn(|conn| {
                if let Ok(count) = commands::deduplication::purge_expired_trash_internal(conn) {
                    if count > 0 {
                        log::info!("Auto-purged {} expired trash files on startup.", count);
                    }
                }
                Ok(())
            });

            // Store database as managed state
            app.manage(database);

            // Initialize background downloader manager
            let downloader = std::sync::Arc::new(services::downloader::DownloadManager::new(
                app.handle().clone(),
            ));
            services::downloader::start_download_worker(
                app.handle().clone(),
                std::sync::Arc::clone(&downloader),
            );
            app.manage(downloader);

            log::info!("VAM Library initialized. Database at: {:?}", app_data_dir);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::game_content::list_game_contents,
            commands::game_content::get_game_content_detail,
            commands::game_content::list_package_scene_contents,
            commands::game_content::save_game_scene_appearance,
            commands::game_content::get_game_content_image,
            commands::game_content::copy_game_presets,
            commands::game_content::set_game_scene_favorite,
            commands::game_content::rename_game_preset,
            commands::game_content::set_game_scene_name,
            commands::game_mods::get_scene_browser_mod_status,
            commands::game_mods::install_scene_browser_mod,
            commands::game_mods::uninstall_scene_browser_mod,
            commands::scan::scan_vam_directory,
            commands::scan::validate_vam_directory,
            commands::scan::start_file_watcher,
            commands::scan::stop_file_watcher,
            commands::packages::get_dashboard_stats,
            commands::packages::find_corrupted_packages,
            commands::packages::list_packages,
            commands::packages::get_package_summary,
            commands::packages::quick_delete_package,
            commands::packages::open_package_in_explorer,
            commands::packages::open_path_in_explorer,
            commands::packages::list_package_folders,
            commands::packages::list_all_tags,
            commands::packages::set_package_tags,
            commands::packages::get_scene_preview,
            commands::packages::get_package_thumbnail,
            commands::packages::list_package_images,
            commands::packages::get_package_image,
            commands::packages::clear_thumbnail_cache,
            commands::dependency::get_dependency_graph,
            commands::dependency::get_package_dependency_relations,
            commands::dependency::get_reverse_dependencies,
            commands::dependency::find_missing_dependencies,
            commands::deduplication::scan_for_duplicates,
            commands::deduplication::get_duplicate_groups,
            commands::deduplication::execute_cleanup,
            commands::deduplication::list_cleanup_trash,
            commands::deduplication::restore_cleanup_trash_item,
            commands::deduplication::delete_cleanup_trash_item,
            commands::deduplication::empty_cleanup_trash,
            commands::migration::preview_migration,
            commands::ingestion::preview_ingestion,
            commands::ingestion::execute_ingestion,
            commands::migration::execute_migration,
            commands::migration::rollback_migration,
            commands::migration::rollback_all_migrations,
            commands::migration::collect_scene_dependencies,
            commands::on_demand::list_on_demand_plans,
            commands::on_demand::save_on_demand_plans,
            commands::on_demand::migrate_on_demand_library,
            commands::on_demand::restore_on_demand_library,
            commands::on_demand::apply_on_demand_plan,
            commands::on_demand::launch_vam_direct,
            commands::on_demand::launch_vam_config,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::save_vam_instances,
            commands::settings::save_hub_auth_cookie,
            commands::settings::get_install_context,
            commands::settings::clear_local_database,
            commands::vam_prefs::read_vam_prefs,
            commands::vam_prefs::save_vam_prefs,
            commands::hub::fetch_hub_package_info,
            commands::hub::fetch_hub_package_info_basic,
            commands::hub::get_hub_login_status,
            commands::hub::browse_hub_packages,
            commands::hub::download_hub_file,
            commands::download::get_download_queue,
            commands::download::add_to_download_queue,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::cancel_download,
            commands::download::retry_download,
            commands::download::clear_completed_downloads,
            commands::download::set_download_settings,
            commands::download::resolve_hub_dependencies,
            commands::download::cancel_hub_dependency_resolution,
            commands::share::get_share_preview,
            commands::share::export_share_zip,
            commands::share::export_installed_packages,
            commands::unpack::analyze_archive,
            commands::unpack::execute_unpack,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
