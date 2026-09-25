//! 构建脚本：给 Windows 可执行文件内嵌一个应用程序清单。
//!
//! 为什么需要它：`native-windows-gui` 会调用 comctl32 的 `SetWindowSubclass` /
//! `GetWindowSubclass` 这一族函数做控件子类化。这几个函数在系统自带的旧版
//! comctl32（v5.82）里**只有序号、没有名字**，只有 v6 才按名字导出。
//! exe 里没有清单时系统只会加载 v5.82，如果链接器恰好把符号解析成了"按名字导入"，
//! 进程启动时就会报 `无法定位程序输入点 GetWindowSubclass`（0xc0000139）。
//!
//! 内嵌一个声明 `Microsoft.Windows.Common-Controls 6.0.0.0` 依赖的清单后，
//! 系统会加载 v6，无论链接器选的是序号还是名字都能解析；顺带还启用了视觉样式。
//! （`new_manifest` 默认就带这条依赖，所以这里不用额外写 XML。）

use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    // 只在为 Windows 目标构建时内嵌；在其他平台上（例如 cargo check）直接跳过，
    // 否则 build script 会因为拿不到 Windows 链接器参数而报错。
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("caplock_remap")).expect("unable to embed manifest file");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
