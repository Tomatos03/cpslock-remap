use interception::{Filter, Interception, KeyFilter, KeyState, ScanCode, Stroke, is_keyboard};
use log::{debug, info};
use std::{
    collections::HashSet,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

use crate::config;
use crate::keymap::{self, Action, CapsLockState, Decision};
use crate::ui::tray::TrayState;

const NO_DEVICE: i32 = 0;
const NO_EVENT: i32 = 0;
const READ_TIMEOUT: Duration = Duration::from_millis(1);

const fn key(code: ScanCode, state: KeyState) -> Stroke {
    Stroke::Keyboard {
        code,
        state,
        information: 0,
    }
}

const CTRL_DOWN: Stroke = key(ScanCode::LeftControl, KeyState::DOWN);
const CTRL_UP: Stroke = key(ScanCode::LeftControl, KeyState::UP);
const ESC_DOWN: Stroke = key(ScanCode::Esc, KeyState::DOWN);
const ESC_UP: Stroke = key(ScanCode::Esc, KeyState::UP);

pub struct Interceptor {
    /// 驱动上下文
    ctx: Interception,
    /// 与托盘线程的收发通道
    sender: Sender<TrayState>,
    receiver: Receiver<TrayState>,
    /// 接收缓冲区；内容每次被 `ctx.receive` 覆盖，初值只是占位
    buf: [Stroke; 1],
    /// CapsLock 状态机的当前状态
    caps_lock_state: CapsLockState,
    /// 托盘线程的状态
    tray_state: TrayState,
    /// 按下过 CapsLock 的设备；退出时需要对它们补发 Ctrl 抬起
    held_devices: HashSet<i32>,
}

impl Interceptor {
    /// 加载驱动并初始化状态。
    ///
    /// 驱动不可用时返回 `None`，由调用方决定如何处理——本模块不终止进程。
    pub fn new(sender: Sender<TrayState>, receiver: Receiver<TrayState>) -> Option<Self> {
        let ctx = Interception::new()?;
        info!("Interception context 创建成功");
        ctx.set_filter(
            is_keyboard,
            Filter::KeyFilter(KeyFilter::UP | KeyFilter::DOWN),
        );

        Some(Self {
            ctx,
            sender,
            receiver,
            buf: [CTRL_DOWN],
            caps_lock_state: CapsLockState::Idle,
            tray_state: TrayState::Runing,
            held_devices: HashSet::new(),
        })
    }

    /// 运行主循环，直到收到退出信号
    pub fn run(&mut self) {
        loop {
            self.poll_messages();
            if self.tray_state == TrayState::Exiting {
                break;
            }

            if let Some(device) = self.next_event_device() {
                self.handle_event(device);
            }
        }

        info!("执行键盘拦截线程后置处理操作");
        self.release_held_ctrl();
        self.tray_state = TrayState::Exited;
        self.sender
            .send(TrayState::Exited)
            .expect("发送退出信号失败");
    }

    /// 处理托盘线程发来的一条消息（若有）
    fn poll_messages(&mut self) {
        let Ok(new_state) = self.receiver.try_recv() else {
            return;
        };

        info!("收到托盘状态变更: {:?} -> {:?}", self.tray_state, new_state);
        if new_state == TrayState::Pause {
            self.release_held_ctrl();
            self.caps_lock_state = CapsLockState::Idle;
        }
        self.tray_state = new_state;
    }

    /// 等待并接收一次设备事件，返回事件来自哪个设备；没有事件时返回 `None`
    fn next_event_device(&mut self) -> Option<i32> {
        let device = self.ctx.wait_with_timeout(READ_TIMEOUT);
        if device == NO_DEVICE {
            return None;
        }
        if self.ctx.receive(device, &mut self.buf) == NO_EVENT {
            return None;
        }
        Some(device)
    }

    /// 处理一次已接收的设备事件
    fn handle_event(&mut self, device: i32) {
        let stroke = self.buf[0];
        let (code, key_state) = match stroke {
            Stroke::Keyboard { code, state, .. } => (code, state),
            // 非键盘事件原样转发
            _ => {
                self.ctx.send(device, &self.buf);
                return;
            }
        };

        // 暂停时全部直通；非 CapsLock 的按键也原样转发
        if self.tray_state == TrayState::Pause || code != ScanCode::CapsLock {
            self.ctx.send(device, &self.buf);
            return;
        }

        self.handle_caps_lock(device, key_state);
    }

    /// 交给状态机决策，并执行它给出的动作
    fn handle_caps_lock(&mut self, device: i32, key_state: KeyState) {
        // 调用方已保证事件来自 CapsLock，这正是状态机要求的调用前提
        let decision = keymap::on_caps_lock_event(
            self.caps_lock_state,
            key_state,
            Instant::now(),
            config::get_threshold(),
        );

        // 进入 Pressed 意味着刚按下 Ctrl，记录设备以便退出时清理
        if matches!(decision.next_state, CapsLockState::Pressed { .. }) {
            self.held_devices.insert(device);
        }

        let Decision {
            next_state,
            actions,
        } = decision;
        debug!(
            "状态转移: {:?} + {:?} -> {:?}, 待执行 {} 个动作",
            self.caps_lock_state,
            key_state,
            next_state,
            actions.len()
        );

        for action in actions {
            self.send_action(device, action);
        }
        self.caps_lock_state = next_state;
    }

    /// 把状态机的动作翻译成驱动能理解的按键序列并发送
    fn send_action(&self, device: i32, action: Action) {
        // 类型标注是必须的：各分支分别是 &[Stroke; 1] 与 &[Stroke; 2]，
        // 长度不同就是不同类型，靠这个标注统一退化成切片 &[Stroke]
        let strokes: &[Stroke] = match action {
            Action::HoldCtrl => &[CTRL_DOWN],
            Action::ReleaseCtrl => &[CTRL_UP],
            Action::TapEsc => &[ESC_DOWN, ESC_UP],
        };
        self.ctx.send(device, strokes);
    }

    /// 对可能残留的 Ctrl 补发抬起，并清空持有记录。
    fn release_held_ctrl(&mut self) {
        // 先整体取出，否则 `send_action` 借用 `self` 会和 `drain` 的可变借用冲突
        let devices = std::mem::take(&mut self.held_devices);
        for device in devices {
            self.send_action(device, Action::ReleaseCtrl);
        }
    }
}
