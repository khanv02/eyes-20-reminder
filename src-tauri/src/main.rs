// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    breakbuddy_lib::run()
}

