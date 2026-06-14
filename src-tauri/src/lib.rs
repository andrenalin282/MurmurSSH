mod commands;
mod models;
mod services;

/// Best-effort teardown run on app exit: stop SSH control sessions, remove any
/// runtime key copies, and clear in-memory session credentials. Mirrors the
/// explicit Disconnect path so closing via the OS title bar leaves no residue.
fn cleanup_on_exit() {
    services::transfer_queue::cancel_all();
    services::ssh_session_service::stop_all_sessions();
    services::runtime_key_service::cleanup_all_runtime_keys();
    services::credentials_store::clear_all();
}

pub fn run() {
    services::launch_service::capture_launch_arg();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            services::transfer_queue::init(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connection::connect_sftp,
            commands::connection::accept_host_key,
            commands::connection::accept_host_key_once,
            commands::connection::save_credential,
            commands::connection::clear_credential,
            commands::connection::clear_session_credentials,
            commands::profile::list_profiles,
            commands::profile::get_profile,
            commands::profile::save_profile,
            commands::profile::delete_profile,
            commands::profile::check_path_exists,
            commands::profile::open_profile_folder,
            commands::profile::get_profiles_path,
            commands::profile::parse_ssh_config,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::ssh::launch_ssh,
            commands::ssh::start_ssh_session,
            commands::ssh::stop_ssh_session,
            commands::ssh::quit_app,
            commands::ssh::open_url,
            commands::ssh::get_app_version,
            commands::ssh::check_key_needs_copy,
            commands::ssh::copy_key_for_runtime,
            commands::ssh::delete_runtime_key,
            commands::ssh::cleanup_runtime_keys,
            commands::launch::get_launch_profile,
            commands::launch::open_profile_in_new_window,
            commands::launch::create_desktop_shortcut,
            commands::sftp::get_sftp_home,
            commands::sftp::list_directory,
            commands::sftp::remote_file_exists,
            commands::sftp::upload_file_bytes,
            commands::sftp::delete_file,
            commands::sftp::delete_directory,
            commands::sftp::rename_file,
            commands::sftp::set_permissions,
            commands::sftp::create_directory,
            commands::sftp::local_file_exists,
            commands::sftp::local_path_is_dir,
            commands::transfer::enqueue_transfer,
            commands::transfer::cancel_transfer,
            commands::transfer::cancel_all_transfers,
            commands::transfer::list_transfers,
            commands::transfer::clear_finished_transfers,
            commands::workspace::open_for_edit,
            commands::local::list_local_directory,
            commands::local::get_home_dir,
            commands::local::get_current_user,
            commands::local::get_local_browser_path,
            commands::local::save_local_browser_path,
            commands::local::rename_local_file,
            commands::local::open_local_file,
        ])
        .build(tauri::generate_context!())
        .expect("error while building MurmurSSH")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                cleanup_on_exit();
            }
        });
}
