use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use crate::ui::threshold_dialog::ThresholdDialog;
use image::GenericImageView;
use log::{info, warn};
use native_windows_gui as nwg;
use tray_icon::TrayIcon;
use tray_icon::menu::MenuEvent;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

/// 退出时等待键盘线程收尾的上限
const EXIT_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

/// 托盘的生命周期状态。
///
/// 托盘线程是它的所有者，键盘线程只是观察者——据此决定按键是直通还是
/// 交给状态机。所以定义放在这里，而不是某个中立的"协议"模块。
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TrayState {
    Pause,
    Runing,
    Exiting,
    Exited,
}

/// `TrayState` 在托盘菜单上的呈现。
impl TrayState {
    /// 暂停/继续开关切到的下一个状态
    fn toggled(&self) -> TrayState {
        match self {
            TrayState::Pause => TrayState::Runing,
            TrayState::Runing => TrayState::Pause,
            // Exiting / Exited 不属于暂停切换的语义范围，保持原有行为
            _ => TrayState::Pause,
        }
    }

    /// 处于该状态时，暂停菜单项应显示的文字
    fn get_menu_text(&self) -> &'static str {
        match self {
            TrayState::Pause => "继续程序",
            _ => "暂停程序",
        }
    }
}

pub struct TrayApp {
    icon: TrayIcon,
    exit: MenuItem,
    pause: MenuItem,
    set_threshold: MenuItem,
    threshold_dialog: ThresholdDialog,
    sender: Sender<TrayState>,
    receiver: Receiver<TrayState>,
    state: TrayState,
}

impl TrayApp {
    pub fn init(sender: Sender<TrayState>, receiver: Receiver<TrayState>) -> Self {
        // 初始化对话弹窗依赖
        nwg::init().expect("Failed to init NWG");

        let pause_item_id = MenuId::new("pause");
        let exit_item_id = MenuId::new("exit");
        let set_threshold_item_id = MenuId::new("set_threshold");

        let pause = MenuItem::with_id(
            pause_item_id.clone(),
            TrayState::Runing.get_menu_text(),
            true,
            None,
        );
        let exit = MenuItem::with_id(exit_item_id.clone(), "退出程序", true, None);
        let set_threshold =
            MenuItem::with_id(set_threshold_item_id.clone(), "设置阈值", true, None);

        let menu = Self::init_menu(&[&set_threshold, &pause, &exit]);

        let icon = TrayIconBuilder::new()
            .with_tooltip("capslock_remap running")
            .with_menu(Box::new(menu))
            .with_icon(Self::init_icon())
            .build()
            .expect("Failed to create tray icon");

        info!("系统托盘图标创建成功");

        Self {
            icon,
            pause,
            exit,
            set_threshold,
            sender,
            receiver,
            state: TrayState::Runing,
            threshold_dialog: ThresholdDialog::default(),
        }
    }

    fn init_menu(menu_items: &[&MenuItem]) -> Menu {
        let menu = Menu::new();
        for item in menu_items.iter() {
            menu.append(*item).expect("Failed to create menu item");
        }
        menu
    }

    fn init_icon() -> Icon {
        let icon_data = include_bytes!("../../asset/icon.ico");

        match image::load_from_memory(icon_data) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = img.dimensions();
                Icon::from_rgba(rgba.into_raw(), width, height).expect("Failed to create icon")
            }
            Err(e) => {
                warn!("图标加载失败: {}, 使用默认图标", e);
                let rgba = vec![255u8; 16 * 16 * 4]; // 16x16 白色图标
                Icon::from_rgba(rgba, 16, 16).expect("Failed to create default icon")
            }
        }
    }

    pub fn handle_exit(&self, event_loop: &ActiveEventLoop) {
        info!("用户点击退出菜单");
        self.sender
            .send(TrayState::Exiting)
            .expect("发送退出信号失败");

        // 等键盘线程完成收尾（补发残留的按键抬起），但设上限：
        // 它若卡住，也不能把事件循环冻死
        if let Err(e) = self.receiver.recv_timeout(EXIT_WAIT_TIMEOUT) {
            warn!("等待键盘线程收尾未成功: {}", e);
        }
        info!("程序即将退出");
        event_loop.exit();
    }

    pub fn handle_pause(&mut self) {
        info!("用户点击暂停/继续菜单，当前状态: {:?}", self.state);

        let new_state = self.state.toggled();
        let new_text = new_state.get_menu_text();

        self.state = new_state.clone();
        self.sender.send(new_state).expect("发送暂停信号失败");
        self.pause.set_text(new_text);
    }

    pub fn handle_set_threshold(&mut self) {
        self.threshold_dialog.open();
    }

    pub fn handle_menu_event(&mut self, event_loop: &ActiveEventLoop) {
        if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            if menu_event.id == self.exit.id() {
                self.handle_exit(event_loop);
            } else if menu_event.id == self.pause.id() {
                self.handle_pause();
            } else if menu_event.id == self.set_threshold.id() {
                self.handle_set_threshold();
            } else {
                warn!("收到未知的菜单事件ID: {:?}", menu_event.id);
            }
        }
    }
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        self.handle_menu_event(event_loop);
    }
}
