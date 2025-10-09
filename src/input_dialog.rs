use std::{sync::mpsc::Sender};
use std::time::Duration;
use native_windows_gui as nwg;
use native_windows_derive as nwd;

use nwd::NwgUi;
use crate::tray::{Message, ThresholdEvent};

#[derive(Default, NwgUi)]
pub struct InputDialog {
    sender: Option<Sender<Message>>,

    #[nwg_control(size: (200, 120), position: (0, 0), title: "修改间隔阈值", flags: "WINDOW|VISIBLE")]
    #[nwg_events( OnWindowClose: [InputDialog::cancel] )]
    pub window: nwg::Window,

    #[nwg_layout(parent: window, spacing: 5)]
    grid: nwg::GridLayout,

    #[nwg_control(size: (0, 0), v_align: nwg::VTextAlign::Center, h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 0, col: 0, col_span: 4)]
    current_threshold_text: nwg::Label,

    #[nwg_control(size: (0, 0))]
    #[nwg_layout_item(layout: grid, row: 1, col: 0, col_span: 4)]
    threshold_edit: nwg::TextInput,

    #[nwg_control(text: "确定")]
    #[nwg_layout_item(layout: grid, row: 2, col: 0, col_span: 2)]
    #[nwg_events( OnButtonClick: [InputDialog::confirm] )]
    cofirm_button: nwg::Button,

    #[nwg_control(text: "取消")]
    #[nwg_layout_item(layout: grid, row: 2, col: 2, col_span: 2)]
    #[nwg_events( OnButtonClick: [InputDialog::cancel] )]
    cancel_button: nwg::Button,
}

impl InputDialog {
    pub fn init_sender(&mut self, sender :Sender<Message>) {
        self.sender = Some(sender);
    }

    pub fn center_window(&self) {
        // 屏幕宽高（像素）
        let sw = nwg::Monitor::width();
        let sh = nwg::Monitor::height();

        // 窗口宽高
        let (ww, wh) = self.window.size();

        // 计算居中位置
        let x = (sw - ww as i32) / 2;
        let y = (sh - wh as i32) / 2;

        self.window.set_position(x.max(0), y.max(0));
    }

    pub fn set_current_threshold_text(&self, current_threshold: Duration) {
        self.current_threshold_text.set_text(&format!("当前阈值: {} ms", current_threshold.as_millis()));
    }

    fn confirm(&self) {
        match self.threshold_edit.text().parse::<u64>() {
            Ok(val) => {
                if val == 0 || val > 30000 {
                    nwg::simple_message("提示", "修改失败, 请输入一个小于30000并且大于0的正整数");
                    return;
                }
                if let Some(sender) = self.sender.as_ref() {
                    match sender.send(Message::ThresholdEvent(ThresholdEvent::Set(Duration::from_millis(val)))) {
                        Ok(_) => {
                            nwg::simple_message("提示", &format!("修改成功, 新阈值: {}ms", val));
                        }
                        Err(e) => { nwg::simple_message("错误", &format!("消息发送失败: {}", e)); }
                    }
                } else {
                    nwg::simple_message("错误", "消息通道未初始化");
                }
                nwg::stop_thread_dispatch();
            },
            Err(_) => {
                nwg::simple_message("提示", "修改失败, 请输入一个正确的正整数");
            }
        }
    }

    fn cancel(&self) {
        nwg::stop_thread_dispatch();
    }
}
