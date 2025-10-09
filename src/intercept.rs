use std::{collections::HashSet, env, sync::mpsc::{Receiver, Sender}, time::{Duration, Instant}};
use interception::{Interception, Stroke, KeyState, ScanCode, Filter, KeyFilter, is_keyboard};
use log::{info, debug, error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::tray::{Message, TrayState, ThresholdEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Pressed {
        since: Instant,
    },
    Holding
}

const NO_DEVICE: i32 = 0;
const NO_EVENT: i32 = 0;
const READ_TIMEOUT: Duration = Duration::from_millis(1);

pub fn intercept_keyboard_thread(sender :Sender<Message>,  receiver :Receiver<Message>) {
    info!("键盘拦截线程启动");
    debug!("尝试创建 interception context");
    let mut threshold = load_config_json();
    let ctx = match Interception::new() {
        Some(ctx) => {
            info!("Interception context 创建成功");
            ctx
        },
        None => {
            error!("创建 interception context 失败");
            std::process::exit(1);
        }
    };
    debug!("设置键盘过滤器: UP | DOWN");
    ctx.set_filter(is_keyboard, Filter::KeyFilter(KeyFilter::UP | KeyFilter::DOWN));
    info!("键盘过滤器设置完成");

    debug!("初始化变量");
    let mut state = State::Idle;
    let mut buf = [Stroke::Keyboard {
        code: ScanCode::A,
        state: KeyState::DOWN,
        information: 0,
    }];
    let mut tray_state = TrayState::Runing;
    let mut last_cps_down_dev_set :HashSet<i32> = HashSet::new();
    info!("键盘拦截循环开始运行");

    loop {
        try_rev_and_handler_message(&sender, &receiver, &mut tray_state, &mut threshold);
        if tray_state == TrayState::Exiting {
            info!("收到退出信号，准备结束键盘拦截线程");
            break;
        }

        let device = ctx.wait_with_timeout(READ_TIMEOUT);
        if device == NO_DEVICE { continue; }
        debug!("检测到设备活动: device={}", device);

        if ctx.receive(device, &mut buf) == NO_EVENT {
            debug!("设备 {} 没有事件", device);
            continue;
        }
        debug!("从设备 {} 收到事件", device);

        let (code, key_state) = match buf[0] {
            Stroke::Keyboard { code, state, .. } => {
                debug!("键盘事件: code={:?}, state={:?}", code, state);
                (code, state)
            },
            _ => {
                debug!("非键盘事件，直接转发");
                ctx.send(device, &buf);
                continue;
            }
        };

        if tray_state == TrayState::Pause || code != ScanCode::CapsLock {
            if tray_state == TrayState::Pause {
                debug!("程序处于暂停状态，直接转发按键");
            }
            ctx.send(device, &buf);
            continue;
        }

        debug!("检测到 CapsLock 按键，当前状态: {:?}", state);
        match (key_state, state) {
            (KeyState::DOWN, State::Idle) => {
                debug!("CapsLock 按下，从 Idle 切换到 Pressed");
                state = State::Pressed {
                    since: Instant::now()
                };
                last_cps_down_dev_set.insert(device);
                debug!("发送 LeftControl DOWN 事件到设备 {}", device);
                ctx.send(device, &[lctrl(KeyState::DOWN)]);
            }
            (KeyState::DOWN, State::Pressed {..}) => {
                debug!("CapsLock 重复按下，切换到 Holding 状态");
                state = State::Holding
            }
            (KeyState::UP, State::Pressed {..}) => {
                debug!("CapsLock 释放，从 Pressed 状态处理");
                debug!("发送 LeftControl UP 事件到设备 {}", device);
                ctx.send(device, &[lctrl(KeyState::UP)]);

                if let State::Pressed { since } = state {
                    let elapsed = since.elapsed();
                    debug!("按键持续时间: {:?}ms, 当前时间阈值: {:?}", elapsed.as_millis(), threshold.as_millis());
                    if elapsed <= threshold {
                        debug!("短按检测，发送 ESC 按键");
                        ctx.send(device, &[esc(KeyState::DOWN)]);
                        ctx.send(device, &[esc(KeyState::UP)]);
                    } else {
                        debug!("长按检测，不发送 ESC");
                    }
                }
                debug!("切换到 Idle 状态");
                state = State::Idle;
            }
            (KeyState::UP, State::Holding) => {
                debug!("CapsLock 释放，从 Holding 状态处理");
                debug!("发送 LeftControl UP 事件到设备 {}", device);
                ctx.send(device, &[lctrl(KeyState::UP)]);
                debug!("切换到 Idle 状态");
                state = State::Idle;
            }
            _ => {
                debug!("未处理的按键状态组合: {:?}, {:?}", key_state, state);
            }
        }
    }

    info!("键盘拦截循环结束，清理持有的 CapsLock 状态");
    clear_holding_caplock(last_cps_down_dev_set, ctx);
    tray_state = TrayState::Exited;
    info!("发送退出完成信号");
    sender.send(Message::TrayState(TrayState::Exited)).expect("发送退出信号失败");
    info!("键盘拦截线程结束，最终状态: {:?}", tray_state);
    store_config_json(threshold);
}

fn clear_holding_caplock(mut set :HashSet<i32>, ctx :Interception) {
    debug!("清理持有的 CapsLock 状态，设备数量: {}", set.len());
    for dev in set.drain() {
        debug!("向设备 {} 发送 LeftControl UP 清理信号", dev);
        ctx.send(dev, &[lctrl(KeyState::UP)]);
    }
    debug!("CapsLock 状态清理完成");
}

fn esc(st: KeyState) -> Stroke {
    Stroke::Keyboard { code: ScanCode::Esc, state: st, information: 0 }
}

fn lctrl(st: KeyState) -> Stroke {
    Stroke::Keyboard { code: ScanCode::LeftControl, state: st, information: 0 }
}

fn try_rev_and_handler_message(sender: &Sender<Message>, receiver :&Receiver<Message>, tray_state: &mut TrayState, current_threshold: &mut Duration) {
    if let Ok(message) = receiver.try_recv() {
        match message {
            Message::TrayState(new_tray_state) => handle_tray_state(tray_state, new_tray_state),
            Message::ThresholdEvent(event) => handle_threshold_event(sender, current_threshold, event),
        }
    }
}

fn handle_tray_state(tray_state: &mut TrayState, new_tray_state: TrayState) {
    info!("收到托盘状态变更: {:?} -> {:?}", tray_state, new_tray_state);
    *tray_state = new_tray_state;
}

fn handle_threshold_event(sender: &Sender<Message>, threshold: &mut Duration, event: ThresholdEvent) {
    match event {
        ThresholdEvent::Get => {
            sender.send(Message::ThresholdEvent(ThresholdEvent::CurrentValue(*threshold)))
                .expect("发送阈值失败");
        }
        ThresholdEvent::Set(new_threshold) => {
            *threshold = new_threshold;
        }
        _ => {}
    }
}

#[derive(Deserialize, Serialize)]
struct Config {
    threshold: u64,
}

fn load_config_json() -> Duration {
    // 获取用户目录
    let user_dir = env::var("USERPROFILE").expect("无法获取用户目录");
    let mut config_path = PathBuf::from(user_dir);
    config_path.push(".caps-remap.json");

    let json_str = match std::fs::read_to_string(&config_path) {
        Ok(t) => t,
        Err(_) => return Duration::from_millis(200),
    };


    // 能解析就取阈值，解析失败也返回默认值
    Duration::from_millis(
        serde_json::from_str::<Config>(&json_str)
            .map(|c| c.threshold)
            .unwrap_or(200) as u64
    )
}

fn store_config_json(threshold: Duration) {
    // 获取用户目录
    let user_dir = env::var("USERPROFILE").expect("无法获取用户目录");
    let mut config_path = PathBuf::from(user_dir);
    config_path.push(".caps-remap.json");

    let config = Config {
        threshold: threshold.as_millis() as u64,
    };

    let json = serde_json::to_string_pretty(&config).expect("序列化配置失败");
    std::fs::write(config_path, json).expect("写入配置文件失败");
}
