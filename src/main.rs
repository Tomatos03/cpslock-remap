#![windows_subsystem = "windows"] // 让Windows识别成Windows应用

mod app;
mod config;
mod intercept;
mod keymap;
mod ui;

use log::info;

fn main() {
    env_logger::init();
    app::run();
}
