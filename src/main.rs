#![windows_subsystem = "windows"] // 让Windows识别成Windows应用

mod intercept;
mod tray;

use intercept::intercept_keyboard_thread;
use tray::Tray;
use tray::TrayState;
use log::{info, debug, warn};

use std::{sync::mpsc::channel, thread};
use winit::event_loop::EventLoop;

fn main() {
    // 初始化日志系统
    env_logger::init();
    info!("程序启动 - caplock_remap v0.1.0");

    debug!("创建通信通道");
    let (tray_sender, intercept_receiver) = channel::<TrayState>();
    let (intercept_sender, tray_receiver) = channel::<TrayState>();
    debug!("通道创建完成");

    info!("开始初始化系统托盘");
    let mut tray = Tray::init_tray(tray_sender, tray_receiver);
    info!("系统托盘初始化完成");

    info!("启动键盘拦截线程");
    thread::spawn(move || {
        debug!("键盘拦截线程开始运行");
        intercept_keyboard_thread(intercept_sender, intercept_receiver);
        warn!("键盘拦截线程结束");
    });

    // 创建事件循环和应用程序处理器
    debug!("创建事件循环");
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    info!("开始事件循环，程序进入运行状态");
    event_loop.run_app(&mut tray).expect("Event loop failed");
    info!("程序退出");
}
