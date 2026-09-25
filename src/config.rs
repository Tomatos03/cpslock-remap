//! 配置持久化。
//!
//! 对外只有两个函数。阈值是进程级配置，首次使用时从
//! %APPDATA%\capslock-remap\config.json 加载，之后只走内存。
//! 位置固定，不支持自定义。

use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use log::warn;
use serde::{Deserialize, Serialize};

const APPDATA_ENV: &str = "APPDATA";
const CONFIG_DIR_NAME: &str = "capslock-remap";
const CONFIG_FILE_NAME: &str = "config.json";
const DEFAULT_THRESHOLD_MS: u64 = 200;

static THRESHOLD_MS: LazyLock<AtomicU64> =
    LazyLock::new(|| AtomicU64::new(read_from_disk().threshold));

/// 当前长按判定阈值
pub fn get_threshold() -> Duration {
    Duration::from_millis(THRESHOLD_MS.load(Ordering::Relaxed))
}

/// 更新阈值并立即落盘
pub fn set_threshold(threshold: Duration) {
    let millis = threshold.as_millis() as u64;
    THRESHOLD_MS.store(millis, Ordering::Relaxed); // 先让读取方看到新值
    write_to_disk(millis); // 再落盘，不阻塞读取方
}

/// 配置文件里的内容
#[derive(Deserialize, Serialize)]
struct Config {
    threshold: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD_MS,
        }
    }
}

/// 配置文件路径；%APPDATA% 不可用时返回 None，不 panic
fn config_path() -> Option<PathBuf> {
    let appdata = env::var_os(APPDATA_ENV)?;
    Some(
        PathBuf::from(appdata)
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME),
    )
}

/// 从磁盘加载；任何一步失败（路径不可用 / 读取失败 / 解析失败）都退回默认配置
fn read_from_disk() -> Config {
    fn try_read() -> Option<Config> {
        let json = std::fs::read_to_string(config_path()?).ok()?;
        serde_json::from_str(&json).ok()
    }

    try_read().unwrap_or_default()
}

/// 写回磁盘；任何一步失败都只记日志，不中断程序
fn write_to_disk(threshold_ms: u64) {
    let file = Config {
        threshold: threshold_ms,
    };
    let json = match serde_json::to_string_pretty(&file) {
        Ok(json) => json,
        Err(e) => {
            warn!("序列化配置失败，本次不写入: {}", e);
            return;
        }
    };
    let Some(path) = config_path() else {
        warn!("无法确定配置路径（%APPDATA% 未设置），本次不写入");
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("创建配置目录失败，本次不写入: {}", e);
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, json) {
        warn!("写入配置文件失败: {}", e);
    }
}
