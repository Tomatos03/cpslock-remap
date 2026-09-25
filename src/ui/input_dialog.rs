use native_windows_derive as nwd;
use native_windows_gui as nwg;
use std::time::Duration;

use crate::config;
use nwd::NwgUi;

#[derive(Default, NwgUi)]
pub struct InputDialog {
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

    /// 把当前阈值显示到窗口上
    pub fn show_current_threshold(&self) {
        self.current_threshold_text.set_text(&format!(
            "当前阈值: {} ms",
            config::get_threshold().as_millis()
        ));
    }

    fn confirm(&self) {
        let Ok(val) = self.threshold_edit.text().parse::<u64>() else {
            nwg::simple_message("提示", "修改失败, 请输入一个正确的正整数");
            return;
        };
        if val == 0 || val > 30000 {
            nwg::simple_message("提示", "修改失败, 请输入一个小于30000并且大于0的正整数");
            return;
        }

        config::set_threshold(Duration::from_millis(val));
        nwg::simple_message("提示", &format!("修改成功, 新阈值: {}ms", val));
        nwg::stop_thread_dispatch();
    }

    fn cancel(&self) {
        nwg::stop_thread_dispatch();
    }
}
