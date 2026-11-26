//! 便携配置：优先 exe 同目录 `winget-picker.ini`，目录不可写时回退
//! `%LOCALAPPDATA%\winget-picker\winget-picker.ini`。
//!
//! 支持环境变量覆盖（测试/高级用户）：
//! - `WINGET_PICKER_INI`      指定 ini 路径
//! - `WINGET_PICKER_THEME`    覆盖主题（auto/light/dark/nord/nord-light）
//! - `WINGET_PICKER_LANG`     覆盖语言（auto/zh/en）
//! - `WINGET_PICKER_SILENT`   覆盖静默升级（1/0）
//! - `WINGET_PICKER_NO_SAVE`  设置后不写盘

use crate::theme;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Auto,
    Zh,
    En,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Auto => "auto",
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Some(Lang::Auto),
            "zh" | "zh-cn" | "zh_cn" => Some(Lang::Zh),
            "en" | "en-us" | "en_us" => Some(Lang::En),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Config {
    pub theme: theme::Choice,
    pub lang: Lang,
    pub silent: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: theme::Choice::Auto,
            lang: Lang::Auto,
            silent: false,
        }
    }
}

fn env_flag(name: &str) -> Option<bool> {
    let v = std::env::var(name).ok()?;
    let v = v.trim().to_ascii_lowercase();
    if matches!(v.as_str(), "1" | "true" | "yes" | "on") {
        Some(true)
    } else if matches!(v.as_str(), "0" | "false" | "no" | "off") {
        Some(false)
    } else {
        None
    }
}

fn no_save() -> bool {
    matches!(std::env::var("WINGET_PICKER_NO_SAVE").ok().as_deref(), Some(v) if !v.is_empty() && v != "0")
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// 目录是否可写（用临时文件探测）。
fn dir_writable(dir: &PathBuf) -> bool {
    let probe = dir.join(".winget-picker-write-test");
    match std::fs::write(&probe, b"x") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn local_appdata_ini() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        std::env::temp_dir().to_string_lossy().into_owned()
    });
    PathBuf::from(base)
        .join("winget-picker")
        .join("winget-picker.ini")
}

/// ini 文件路径。
pub fn ini_path() -> PathBuf {
    if let Ok(p) = std::env::var("WINGET_PICKER_INI") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Some(dir) = exe_dir() {
        let candidate = dir.join("winget-picker.ini");
        if candidate.exists() || dir_writable(&dir) {
            return candidate;
        }
    }
    local_appdata_ini()
}

impl Config {
    pub fn load() -> Self {
        let mut cfg = Config::default();
        let path = ini_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with('[')
                {
                    continue;
                }
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let key = key.trim();
                let value = value.trim();
                match key {
                    "theme" => {
                        if let Some(c) = theme::Choice::parse(value) {
                            cfg.theme = c;
                        }
                    }
                    "lang" => {
                        if let Some(l) = Lang::parse(value) {
                            cfg.lang = l;
                        }
                    }
                    "silent" => {
                        cfg.silent = matches!(
                            value.to_ascii_lowercase().as_str(),
                            "1" | "true" | "yes" | "on"
                        );
                    }
                    _ => {}
                }
            }
        }
        if let Ok(v) = std::env::var("WINGET_PICKER_THEME") {
            if let Some(c) = theme::Choice::parse(&v) {
                cfg.theme = c;
            }
        }
        if let Ok(v) = std::env::var("WINGET_PICKER_LANG") {
            if let Some(l) = Lang::parse(&v) {
                cfg.lang = l;
            }
        }
        if let Some(b) = env_flag("WINGET_PICKER_SILENT") {
            cfg.silent = b;
        }
        cfg
    }

    /// 保存（失败静默忽略）。返回是否真的写了文件。
    pub fn save(&self) -> bool {
        if no_save() {
            return false;
        }
        let path = ini_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = format!(
            "theme={}\nlang={}\nsilent={}\n",
            self.theme.as_str(),
            self.lang.as_str(),
            if self.silent { 1 } else { 0 }
        );
        std::fs::write(&path, text).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lang_and_theme() {
        assert_eq!(Lang::parse("zh"), Some(Lang::Zh));
        assert_eq!(Lang::parse("EN"), Some(Lang::En));
        assert_eq!(Lang::parse("xx"), None);
        assert_eq!(theme::Choice::parse("nord-light"), Some(theme::Choice::NordLight));
        assert_eq!(theme::Choice::parse("dark"), Some(theme::Choice::Dark));
        assert_eq!(theme::Choice::Auto.resolve_index(true), 1);
        assert_eq!(theme::Choice::Auto.resolve_index(false), 0);
    }
}
