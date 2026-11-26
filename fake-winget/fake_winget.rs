// 测试用假 winget：列出时输出真实捕获的 winget 表格；升级时记录参数并模拟成功/失败。
use std::io::Write;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(dir) = &exe_dir {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("calls.log"))
        {
            let _ = writeln!(f, "{}", args.join(" "));
        }
    }

    let listing = !args.iter().any(|a| a == "--id");
    if listing {
        // 可选：模拟慢速列表（spinner 截图测试用）
        if let Ok(v) = std::env::var("FAKE_WINGET_LIST_DELAY_MS") {
            if let Ok(ms) = v.parse::<u64>() {
                std::thread::sleep(Duration::from_millis(ms));
            }
        }
        if let Some(dir) = &exe_dir {
            if let Ok(text) = std::fs::read_to_string(dir.join("fixture.txt")) {
                print!("{}", text);
            }
        }
        return;
    }

    let id = args
        .iter()
        .position(|a| a == "--id")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    let silent = args.iter().any(|a| a == "--silent");
    std::thread::sleep(Duration::from_millis(400));
    // 模拟“静默模式失败、普通模式成功”，用于验证自动回退
    if silent && id == "Git.Git" {
        eprintln!("模拟：该安装器不支持静默升级。");
        std::process::exit(3);
    }
    if id == "CreativeTechnology.OpenAL" {
        eprintln!("安装程序哈希校验失败，升级已中止。");
        eprintln!("winget 返回错误（0x8A15002B）。");
        std::process::exit(1);
    }
    println!("已成功升级 {id}");
}
