use std::path::{Path, PathBuf};
use std::process::Command;

/// 找 rc.exe：WINGET_PICKER_RC 覆盖 → PATH → Windows Kits（取最新版本）。
fn find_rc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("WINGET_PICKER_RC") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(';') {
            if dir.trim().is_empty() {
                continue;
            }
            let candidate = Path::new(dir).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let kits = Path::new("C:\\Program Files (x86)\\Windows Kits\\10\\bin");
    let mut best: Option<(String, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(kits) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("x64").join("rc.exe");
            if candidate.is_file() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if best.as_ref().map(|(n, _)| name > *n).unwrap_or(true) {
                    best = Some((name, candidate));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("app.manifest");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );

    // 图标 + 版本资源（assets/app.rc 存在时才编译；没有 rc.exe 时自动跳过，不影响构建）
    let assets = manifest_dir.join("assets");
    let rc_file = assets.join("app.rc");
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");
    if !rc_file.is_file() {
        return;
    }
    let Some(rc_exe) = find_rc() else {
        println!("cargo:warning=未找到 rc.exe，跳过图标资源（可设置 WINGET_PICKER_RC 指定路径）");
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("app.res");
    let status = Command::new(&rc_exe)
        .current_dir(&assets)
        .arg("/nologo")
        .arg("/fo")
        .arg(&out)
        .arg("app.rc")
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={}", out.display());
        }
        Ok(s) => println!("cargo:warning=rc.exe 返回 {s:?}，跳过图标资源"),
        Err(e) => println!("cargo:warning=无法运行 rc.exe（{e}），跳过图标资源"),
    }
}
