//! 界面层。
//!
//! 托盘图标、菜单、对话框——所有直接和用户打交道的东西。
//! 本层定义托盘状态 `TrayState`，键盘线程据此决定按键是直通还是交给状态机。

pub mod input_dialog;
pub mod threshold_dialog;
pub mod tray;
