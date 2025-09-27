use std::sync::mpsc::{Receiver, Sender};

use tray_icon::TrayIcon;
use winit::{application::ApplicationHandler};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;
use tray_icon::menu::MenuEvent;
use image::GenericImageView;
use log::{info, debug, warn};

use tray_icon::{
    menu::{Menu, MenuItem, MenuId},
    TrayIconBuilder, Icon,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TrayState {
    Pause,
    Runing,
    Exiting,
    Exited,
}

pub struct Tray {
    _tray_icon : TrayIcon,
    exit_item: MenuItem,
    pause_item: MenuItem,
    sender: Sender<TrayState>,
    receiver: Receiver<TrayState>,
    state: TrayState,
}


impl Tray {
    pub fn init_tray(sender :Sender<TrayState>, receiver :Receiver<TrayState>) -> Self {
        debug!("开始创建系统托盘组件");

        debug!("创建菜单项ID");
        let pause_item_id = MenuId::new("pause");
        let exit_item_id = MenuId::new("exit");
        debug!("菜单项ID创建完成: pause={:?}, exit={:?}", pause_item_id, exit_item_id);

        debug!("创建菜单项");
        let pause_item = MenuItem::with_id(pause_item_id.clone(), "暂停程序", true, None);
        let exit_item = MenuItem::with_id(exit_item_id.clone(), "退出程序", true, None);
        debug!("菜单项创建完成");

        debug!("开始创建托盘菜单");
        let menu = Self::init_menu(&[&pause_item, &exit_item]);
        debug!("托盘菜单创建完成");

        debug!("开始创建系统托盘图标");
        let _tray_icon = TrayIconBuilder::new().with_tooltip("capslock_remap running")
                                                       .with_menu(Box::new(menu))
                                                       .with_icon(Self::init_icon())
                                                       .build()
                                                       .expect("Failed to create tray icon");
        info!("系统托盘图标创建成功");

        debug!("系统托盘结构体初始化完成");
        Self {
            _tray_icon,
            pause_item,
            exit_item,
            sender,
            receiver,
            state: TrayState::Runing
        }
    }

    fn init_menu(menu_items :&[&MenuItem]) -> Menu {
        debug!("创建新的菜单对象");
        let menu = Menu::new();
        debug!("开始添加菜单项，共{}项", menu_items.len());
        for (index, item) in menu_items.iter().enumerate() {
            debug!("添加菜单项 {}/{}", index + 1, menu_items.len());
            menu.append(*item).expect("Failed to create menu item");
        }
        debug!("所有菜单项添加完成");
        menu
    }

    fn init_icon() -> Icon{
        debug!("开始加载系统托盘图标");
        let icon_data = include_bytes!("../res/icon.ico");
        debug!("图标文件大小: {} 字节", icon_data.len());

        match image::load_from_memory(icon_data) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = img.dimensions();
                info!("图标加载成功: {}x{} pixels", width, height);
                Icon::from_rgba(rgba.into_raw(), width, height).expect("Failed to create icon")
            }
            Err(e) => {
                warn!("图标加载失败: {}, 使用默认图标", e);
                let rgba = vec![255u8; 16 * 16 * 4]; // 16x16 白色图标
                debug!("创建默认白色图标 16x16");
                Icon::from_rgba(rgba, 16, 16).expect("Failed to create default icon")
            }
        }
    }

    pub fn handle_exit_menu(&self, event_loop: &ActiveEventLoop) {
        info!("用户点击退出菜单");
        debug!("发送退出信号");
        self.sender.send(TrayState::Exiting).expect("发送退出信号失败");
        debug!("等待退出确认");
        self.receiver.recv().expect("等待退出信号失败");
        info!("程序即将退出");
        event_loop.exit();
    }

    pub fn handle_pause_menu(&mut self) {
        info!("用户点击暂停/继续菜单，当前状态: {:?}", self.state);
        let new_state = if self.state == TrayState::Pause { TrayState::Runing } else { TrayState::Pause };
        let new_text = if new_state == TrayState::Pause { "继续程序" } else { "暂停程序" };
        info!("切换到新状态: {:?}，菜单文本: {}", new_state, new_text);

        debug!("发送状态变更信号");
        self.state = new_state.clone();
        self.sender.send(new_state).expect("发送暂停信号失败");
        debug!("更新菜单文本");
        self.pause_item.set_text(new_text);
        debug!("状态切换完成");
    }

}

impl ApplicationHandler for Tray {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        debug!("应用程序恢复运行");
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
        debug!("窗口事件: {:?}", _event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);

        // 检查菜单事件
        if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            debug!("收到菜单事件: {:?}", menu_event.id);
            if menu_event.id == self.exit_item.id() {
                debug!("识别为退出菜单事件");
                self.handle_exit_menu(event_loop);
            } else if menu_event.id == self.pause_item.id() {
                debug!("识别为暂停菜单事件");
                self.handle_pause_menu();
            } else {
                warn!("收到未知的菜单事件ID: {:?}", menu_event.id);
            }
        }
    }
}
