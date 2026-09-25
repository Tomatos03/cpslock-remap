//! 应用装配（组合根）。
//!
//! 这里是全项目唯一知道"程序由哪些模块组成、它们如何接线"的地方。
//! 各模块之间通过 `ui::tray::TrayState` 通信，彼此不认识对方。

use std::sync::mpsc::channel;
use std::thread;

use log::{error, info};
use winit::event_loop::EventLoop;

use crate::intercept::Interceptor;
use crate::ui::tray::{TrayApp, TrayState};

pub fn run() {
    // tray 线程 --> intercept 线程
    let (tray_sender, intercept_receiver) = channel::<TrayState>();
    // intercept 线程 --> tray 线程
    let (intercept_sender, tray_receiver) = channel::<TrayState>();

    start_interceptor_thread(intercept_receiver, intercept_sender);
    tray_app_event_loop(tray_sender, tray_receiver);
}

fn start_interceptor_thread(
    intercept_receiver: std::sync::mpsc::Receiver<TrayState>,
    intercept_sender: std::sync::mpsc::Sender<TrayState>,
) {
    thread::spawn(move || {
        info!("键盘拦截线程启动");

        let Some(mut interceptor) = Interceptor::new(intercept_sender, intercept_receiver) else {
            error!("创建 interception context 失败");
            std::process::exit(1);
        };
        interceptor.run();
        info!("键盘拦截线程结束");
    });
}

fn tray_app_event_loop(
    tray_sender: std::sync::mpsc::Sender<TrayState>,
    tray_receiver: std::sync::mpsc::Receiver<TrayState>,
) {
    info!("托盘启动");
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut tray = TrayApp::init(tray_sender, tray_receiver);
    event_loop
        .run_app(&mut tray)
        .expect("Event loop start failed");
    info!("托盘退出");
}
