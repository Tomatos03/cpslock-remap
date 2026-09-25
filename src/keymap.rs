//! CapsLock 映射状态机。
//!
//! 纯逻辑：不接触驱动、不读时钟、不打日志。
//! 输出用领域语言（`Action`）描述，不含扫描码——翻译成具体按键由驱动层负责。

use std::time::{Duration, Instant};

use interception::KeyState;

/// 状态机要求驱动执行的动作，用领域语言描述
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 按住左 Ctrl
    HoldCtrl,
    /// 松开左 Ctrl
    ReleaseCtrl,
    /// 补一次 ESC 点击
    TapEsc,
}

/// CapsLock 处理状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsLockState {
    /// 未按下
    Idle,
    /// 已按下且尚未判定短按/长按
    Pressed { since: Instant },
    /// 按住过程中又收到按下事件（键盘自动重复）
    Holding,
}

/// 状态机对一次 CapsLock 事件的决策
#[derive(Debug)]
pub struct Decision {
    /// 转移后的新状态
    pub next_state: CapsLockState,
    /// 需要依次执行的动作
    pub actions: Vec<Action>,
}

/// 处理一次 **CapsLock** 事件；
pub fn on_caps_lock_event(
    state: CapsLockState,
    key_state: KeyState,
    now: Instant,
    threshold: Duration,
) -> Decision {
    match (key_state, state) {
        (KeyState::DOWN, CapsLockState::Idle) => Decision {
            next_state: CapsLockState::Pressed { since: now },
            actions: vec![Action::HoldCtrl],
        },
        (KeyState::DOWN, CapsLockState::Pressed { .. }) => Decision {
            next_state: CapsLockState::Holding,
            actions: Vec::new(),
        },
        (KeyState::UP, CapsLockState::Pressed { since }) => {
            let mut actions = vec![Action::ReleaseCtrl];
            // 短按补一次 ESC 点击，长按不补
            if now.duration_since(since) <= threshold {
                actions.push(Action::TapEsc);
            }
            Decision {
                next_state: CapsLockState::Idle,
                actions,
            }
        }
        (KeyState::UP, CapsLockState::Holding) => Decision {
            next_state: CapsLockState::Idle,
            actions: vec![Action::ReleaseCtrl],
        },
        // (DOWN, Holding) 与 (UP, Idle)：保持原状态，不做任何动作
        _ => Decision {
            next_state: state,
            actions: Vec::new(),
        },
    }
}
