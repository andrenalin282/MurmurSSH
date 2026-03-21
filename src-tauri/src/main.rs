// Hides the console window on Windows release builds. No effect on Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    murmurssh_lib::run();
}
