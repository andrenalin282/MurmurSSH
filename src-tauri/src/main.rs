// Hides the console window on Windows release builds. No effect on Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Disable DMABUF renderer to fix WebKit crashes on Wayland when running as AppImage.
    // Harmless on X11 and .deb installs — only disables a GPU memory-sharing optimisation.
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    murmurssh_lib::run();
}
