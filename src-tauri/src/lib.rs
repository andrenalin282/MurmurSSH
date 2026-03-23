mod commands;
mod models;
mod services;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            commands::sftp::get_sftp_home,
            commands::sftp::list_directory,
            commands::sftp::upload_file,
            commands::sftp::upload_file_bytes,
            commands::sftp::download_file,
            commands::sftp::download_file_to,
            commands::sftp::delete_file,
            commands::sftp::delete_directory,
            commands::sftp::upload_directory,
            commands::sftp::download_directory,
            commands::sftp::rename_file,
            commands::sftp::create_directory,
            commands::workspace::open_for_edit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MurmurSSH");
}
