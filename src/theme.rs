//! 主题色板与主题选择。
//!
//! 5 套选项（跟随系统 / 浅色 / 深色 / Nord 深色 / Nord 浅色），实际绘制只用 4 套色板；
//! “跟随系统”在启动/切换时解析成浅色或深色。

use std::sync::atomic::{AtomicUsize, Ordering};

pub const fn rgbc(r: u32, g: u32, b: u32) -> u32 {
    (r & 0xFF) | ((g & 0xFF) << 8) | ((b & 0xFF) << 16)
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: u32,
    pub list_bg: u32,
    pub header_bg: u32,
    pub border: u32,
    pub grid: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub ok: u32,
    pub err: u32,
    pub btn_bg: u32,
    pub btn_hover: u32,
    pub btn_pressed: u32,
    pub btn_text: u32,
    pub btn_disabled_text: u32,
    pub progress_track: u32,
}

/// 浅色（取自 Beluga CSV 的 light token）
pub const LIGHT: Theme = Theme {
    dark: false,
    bg: rgbc(0xf5, 0xf6, 0xf8),
    list_bg: rgbc(0xff, 0xff, 0xff),
    header_bg: rgbc(0xf2, 0xf4, 0xf7),
    border: rgbc(0xe2, 0xe6, 0xeb),
    grid: rgbc(0xee, 0xf0, 0xf3),
    text: rgbc(0x1f, 0x23, 0x28),
    muted: rgbc(0x6b, 0x72, 0x80),
    accent: rgbc(0x25, 0x63, 0xeb),
    accent_soft: rgbc(0xee, 0xf3, 0xff),
    ok: rgbc(0x15, 0x80, 0x3d),
    err: rgbc(0xb4, 0x23, 0x18),
    btn_bg: rgbc(0xff, 0xff, 0xff),
    btn_hover: rgbc(0xf2, 0xf4, 0xf7),
    btn_pressed: rgbc(0xe2, 0xe6, 0xeb),
    btn_text: rgbc(0x1f, 0x23, 0x28),
    btn_disabled_text: rgbc(0x9a, 0xa3, 0xaf),
    progress_track: rgbc(0xe2, 0xe6, 0xeb),
};

/// 深色（Beluga CSV dark token）
pub const DARK: Theme = Theme {
    dark: true,
    bg: rgbc(0x14, 0x17, 0x1c),
    list_bg: rgbc(0x1b, 0x1f, 0x26),
    header_bg: rgbc(0x22, 0x27, 0x2f),
    border: rgbc(0x2c, 0x33, 0x3d),
    grid: rgbc(0x26, 0x2c, 0x34),
    text: rgbc(0xe6, 0xe9, 0xee),
    muted: rgbc(0x9a, 0xa4, 0xb2),
    accent: rgbc(0x3b, 0x82, 0xf6),
    accent_soft: rgbc(0x1b, 0x27, 0x40),
    ok: rgbc(0x7e, 0xc6, 0x99),
    err: rgbc(0xf0, 0x71, 0x78),
    btn_bg: rgbc(0x22, 0x27, 0x2f),
    btn_hover: rgbc(0x28, 0x2e, 0x38),
    btn_pressed: rgbc(0x1b, 0x1f, 0x26),
    btn_text: rgbc(0xe6, 0xe9, 0xee),
    btn_disabled_text: rgbc(0x6f, 0x78, 0x85),
    progress_track: rgbc(0x26, 0x2c, 0x34),
};

/// Nord 深色（Beluga CSV nord token）
pub const NORD: Theme = Theme {
    dark: true,
    bg: rgbc(0x2e, 0x34, 0x40),
    list_bg: rgbc(0x2b, 0x31, 0x3d),
    header_bg: rgbc(0x3b, 0x42, 0x52),
    border: rgbc(0x4c, 0x56, 0x6a),
    grid: rgbc(0x3b, 0x42, 0x52),
    text: rgbc(0xd8, 0xde, 0xe9),
    muted: rgbc(0x99, 0xa1, 0xb0),
    accent: rgbc(0x88, 0xc0, 0xd0),
    accent_soft: rgbc(0x3b, 0x42, 0x52),
    ok: rgbc(0xa3, 0xbe, 0x8c),
    err: rgbc(0xbf, 0x61, 0x6a),
    btn_bg: rgbc(0x3b, 0x42, 0x52),
    btn_hover: rgbc(0x43, 0x4c, 0x5e),
    btn_pressed: rgbc(0x2e, 0x34, 0x40),
    btn_text: rgbc(0xd8, 0xde, 0xe9),
    btn_disabled_text: rgbc(0x7b, 0x84, 0x96),
    progress_track: rgbc(0x3b, 0x42, 0x52),
};

/// Nord 浅色（Beluga CSV nord-light token）
pub const NORD_LIGHT: Theme = Theme {
    dark: false,
    bg: rgbc(0xe5, 0xe9, 0xf0),
    list_bg: rgbc(0xf1, 0xf4, 0xf8),
    header_bg: rgbc(0xec, 0xef, 0xf4),
    border: rgbc(0xd8, 0xde, 0xe9),
    grid: rgbc(0xdf, 0xe4, 0xec),
    text: rgbc(0x2e, 0x34, 0x40),
    muted: rgbc(0x4c, 0x56, 0x6a),
    accent: rgbc(0x5e, 0x81, 0xac),
    accent_soft: rgbc(0xdb, 0xe3, 0xef),
    ok: rgbc(0x55, 0x70, 0x3f),
    err: rgbc(0xa9, 0x4f, 0x58),
    btn_bg: rgbc(0xec, 0xef, 0xf4),
    btn_hover: rgbc(0xdb, 0xe3, 0xef),
    btn_pressed: rgbc(0xd8, 0xde, 0xe9),
    btn_text: rgbc(0x2e, 0x34, 0x40),
    btn_disabled_text: rgbc(0x6f, 0x7a, 0x8c),
    progress_track: rgbc(0xd8, 0xde, 0xe9),
};

pub const THEMES: [Theme; 4] = [LIGHT, DARK, NORD, NORD_LIGHT];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Auto,
    Light,
    Dark,
    Nord,
    NordLight,
}

pub const CHOICES: [Choice; 5] = [
    Choice::Auto,
    Choice::Light,
    Choice::Dark,
    Choice::Nord,
    Choice::NordLight,
];

impl Choice {
    pub fn as_str(self) -> &'static str {
        match self {
            Choice::Auto => "auto",
            Choice::Light => "light",
            Choice::Dark => "dark",
            Choice::Nord => "nord",
            Choice::NordLight => "nord-light",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Some(Choice::Auto),
            "light" => Some(Choice::Light),
            "dark" => Some(Choice::Dark),
            "nord" => Some(Choice::Nord),
            "nord-light" | "nord_light" | "nordlight" => Some(Choice::NordLight),
            _ => None,
        }
    }

    /// 按钮序号（0..5）
    pub fn index(self) -> usize {
        match self {
            Choice::Auto => 0,
            Choice::Light => 1,
            Choice::Dark => 2,
            Choice::Nord => 3,
            Choice::NordLight => 4,
        }
    }

    pub fn from_index(i: usize) -> Option<Self> {
        CHOICES.get(i).copied()
    }

    /// 解析成实际色板下标（0 浅色 / 1 深色 / 2 Nord / 3 Nord 浅色）
    pub fn resolve_index(self, system_dark: bool) -> usize {
        match self {
            Choice::Auto => {
                if system_dark {
                    1
                } else {
                    0
                }
            }
            Choice::Light => 0,
            Choice::Dark => 1,
            Choice::Nord => 2,
            Choice::NordLight => 3,
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Choice::Auto => "跟随系统",
            Choice::Light => "浅色",
            Choice::Dark => "深色",
            Choice::Nord => "Nord 深色",
            Choice::NordLight => "Nord 浅色",
        }
    }

    pub fn label_en(self) -> &'static str {
        match self {
            Choice::Auto => "Follow system",
            Choice::Light => "Light",
            Choice::Dark => "Dark",
            Choice::Nord => "Nord dark",
            Choice::NordLight => "Nord light",
        }
    }
}

static ACTIVE_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn active() -> &'static Theme {
    &THEMES[ACTIVE_INDEX.load(Ordering::Relaxed) % THEMES.len()]
}

pub fn set_active_index(index: usize) {
    ACTIVE_INDEX.store(index % THEMES.len(), Ordering::Relaxed);
}

// ---------------------------------------------------------------- 系统深色检测

type HKEY = isize;
const HKEY_CURRENT_USER: HKEY = 0x8000_0001u32 as i32 as isize;
const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        hkey: HKEY,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        value_type: *mut u32,
        data: *mut std::ffi::c_void,
        data_size: *mut u32,
    ) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 读取系统“应用使用浅色主题”的设置；读不到时按浅色处理。
pub fn system_uses_dark() -> bool {
    let sub = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value = wide("AppsUseLightTheme");
    let mut data: u32 = 1;
    let mut size: u32 = 4;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut std::ffi::c_void,
            &mut size,
        )
    };
    rc == 0 && data == 0
}
