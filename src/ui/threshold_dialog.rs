//! 阈值设置对话框的调度。
//!
//! 在独立线程中弹出对话框，并保证同一时刻最多只有一个在运行。
//! 阈值本身由 `config` 模块持有，本模块不参与它的读写。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::debug;
use native_windows_gui as nwg;
use nwg::NativeUi;

use crate::ui::input_dialog::InputDialog;

/// 阈值对话框调度器
#[derive(Default)]
pub struct ThresholdDialog {
    /// 是否有对话框正在运行；用 `Arc` 是因为它要跟着线程一起移动
    open: Arc<AtomicBool>,
}

impl ThresholdDialog {
    /// 弹出对话框；对话框在独立线程中运行，本函数立即返回
    pub fn open(&self) {
        // swap 返回旧值。检查与设置必须是一个原子操作，否则两次快速点击都会
        // 读到 false，从而弹出两个对话框
        if self.open.swap(true, Ordering::SeqCst) {
            debug!("已经有一个设置阈值的对话框在运行，忽略新的请求");
            return;
        }

        let open = Arc::clone(&self.open);
        std::thread::spawn(move || {
            let app = InputDialog::build_ui(InputDialog::default()).expect("Failed to build UI");
            app.center_window();
            app.show_current_threshold();

            nwg::dispatch_thread_events();
            open.store(false, Ordering::SeqCst);
        });
    }
}
