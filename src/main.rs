// winget-picker — 小型 winget 选择性升级工具
//
// 初始界面：查看升级 / 退出
// 查看升级：后台执行 `winget upgrade`，解析表格，按名称排序后以清单显示，
//           勾选（可全选）后后台逐个执行 `winget upgrade --id ...`，
//           一轮结束后回到初始界面。
//
// 设计目标：单文件、零第三方依赖、二进制尽量小（只调用系统 DLL）。

#![cfg_attr(all(not(test), not(debug_assertions)), windows_subsystem = "windows")]
#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;

// ---------------------------------------------------------------- Win32 类型

type HWND = *mut c_void;
type HINSTANCE = *mut c_void;
type HMENU = *mut c_void;
type HICON = *mut c_void;
type HCURSOR = *mut c_void;
type HBRUSH = *mut c_void;
type HFONT = *mut c_void;
type HGDIOBJ = *mut c_void;
type HDC = *mut c_void;
type HMODULE = *mut c_void;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;
type BOOL = i32;
type UINT = u32;
type DWORD = u32;
type ATOM = u16;
type LPCWSTR = *const u16;
type LPWSTR = *mut u16;
type UINT_PTR = usize;
type WNDPROC = unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT;
type TIMERPROC = Option<unsafe extern "system" fn(HWND, UINT, UINT_PTR, DWORD)>;

macro_rules! zero_default {
    ($t:ty) => {
        impl Default for $t {
            fn default() -> Self {
                unsafe { std::mem::zeroed() }
            }
        }
    };
}

#[repr(C)]
#[derive(Clone, Copy)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MSG {
    hwnd: HWND,
    message: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
    time: DWORD,
    pt: POINT,
}
zero_default!(MSG);

#[repr(C)]
struct WNDCLASSEXW {
    cb_size: UINT,
    style: UINT,
    lpfn_wnd_proc: Option<WNDPROC>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: HINSTANCE,
    h_icon: HICON,
    h_cursor: HCURSOR,
    hbr_background: HBRUSH,
    lpsz_menu_name: LPCWSTR,
    lpsz_class_name: LPCWSTR,
    h_icon_sm: HICON,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LOGFONTW {
    lf_height: i32,
    lf_width: i32,
    lf_escapement: i32,
    lf_orientation: i32,
    lf_weight: i32,
    lf_italic: u8,
    lf_underline: u8,
    lf_strike_out: u8,
    lf_char_set: u8,
    lf_out_precision: u8,
    lf_clip_precision: u8,
    lf_quality: u8,
    lf_pitch_and_family: u8,
    lf_face_name: [u16; 32],
}
zero_default!(LOGFONTW);

#[repr(C)]
#[derive(Clone, Copy)]
struct NONCLIENTMETRICS {
    cb_size: UINT,
    i_border_width: i32,
    i_scroll_width: i32,
    i_scroll_height: i32,
    i_caption_width: i32,
    i_caption_height: i32,
    lf_caption_font: LOGFONTW,
    i_sm_caption_width: i32,
    i_sm_caption_height: i32,
    lf_sm_caption_font: LOGFONTW,
    i_menu_width: i32,
    i_menu_height: i32,
    lf_menu_font: LOGFONTW,
    lf_status_font: LOGFONTW,
    lf_message_font: LOGFONTW,
    i_padded_border_width: i32,
}
zero_default!(NONCLIENTMETRICS);

#[repr(C)]
struct INITCOMMONCONTROLSEX {
    dw_size: DWORD,
    dw_icc: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LVCOLUMNW {
    mask: UINT,
    fmt: i32,
    cx: i32,
    psz_text: *const u16,
    cch_text: i32,
    i_sub_item: i32,
    i_image: i32,
    i_order: i32,
    cx_min: i32,
    cx_default: i32,
    cx_ideal: i32,
}
zero_default!(LVCOLUMNW);

#[repr(C)]
#[derive(Clone, Copy)]
struct LVITEMW {
    mask: UINT,
    i_item: i32,
    i_sub_item: i32,
    state: UINT,
    state_mask: UINT,
    psz_text: *mut u16,
    cch_text: i32,
    i_image: i32,
    l_param: LPARAM,
    i_indent: i32,
    i_group_id: i32,
    c_columns: UINT,
    pu_columns: *mut UINT,
    pi_col_fmt: *mut i32,
    i_group: i32,
}
zero_default!(LVITEMW);

#[repr(C)]
#[derive(Clone, Copy)]
struct NMHDR {
    hwnd_from: HWND,
    id_from: UINT_PTR,
    code: UINT,
}
zero_default!(NMHDR);

#[repr(C)]
#[derive(Clone, Copy)]
struct NMLISTVIEW {
    hdr: NMHDR,
    i_item: i32,
    i_sub_item: i32,
    u_new_state: UINT,
    u_old_state: UINT,
    l_param: LPARAM,
}
zero_default!(NMLISTVIEW);

// ---------------------------------------------------------------- Win32 常量

const WM_DESTROY: UINT = 0x0002;
const WM_CLOSE: UINT = 0x0010;
const WM_SETFONT: UINT = 0x0030;
const WM_COMMAND: UINT = 0x0111;
const WM_TIMER: UINT = 0x0113;
const WM_NOTIFY: UINT = 0x004E;
const WM_KEYDOWN: UINT = 0x0100;
const WM_CTLCOLORSTATIC: UINT = 0x0138;

const WS_OVERLAPPED: DWORD = 0x0000_0000;
const WS_CAPTION: DWORD = 0x00C0_0000;
const WS_SYSMENU: DWORD = 0x0008_0000;
const WS_MINIMIZEBOX: DWORD = 0x0002_0000;
const WS_CLIPCHILDREN: DWORD = 0x0200_0000;
const WS_CHILD: DWORD = 0x4000_0000;
const WS_VISIBLE: DWORD = 0x1000_0000;
const WS_TABSTOP: DWORD = 0x0001_0000;
const WS_EX_CLIENTEDGE: DWORD = 0x0000_0200;
const BS_PUSHBUTTON: DWORD = 0x0000_0000;
const SS_LEFT: DWORD = 0x0000_0000;
const SS_CENTER: DWORD = 0x0000_0001;
const SS_NOPREFIX: DWORD = 0x0000_0080;

const LVS_REPORT: DWORD = 0x0001;
const LVS_SHOWSELALWAYS: DWORD = 0x0008;
const LVS_EX_GRIDLINES: DWORD = 0x0000_0001;
const LVS_EX_CHECKBOXES: DWORD = 0x0000_0004;
const LVS_EX_FULLROWSELECT: DWORD = 0x0000_0020;
const LVS_EX_DOUBLEBUFFER: DWORD = 0x0001_0000;

const LVM_FIRST: UINT = 0x1000;
const LVM_GETITEMCOUNT: UINT = LVM_FIRST + 4;
const LVM_DELETEALLITEMS: UINT = LVM_FIRST + 9;
const LVM_GETHEADER: UINT = LVM_FIRST + 31;
const LVM_SETITEMSTATE: UINT = LVM_FIRST + 43;
const LVM_GETITEMSTATE: UINT = LVM_FIRST + 44;
const LVM_SETEXTENDEDLISTVIEWSTYLE: UINT = LVM_FIRST + 54;
const LVM_INSERTITEMW: UINT = LVM_FIRST + 77;
const LVM_INSERTCOLUMNW: UINT = LVM_FIRST + 97;
const LVM_SETITEMTEXTW: UINT = LVM_FIRST + 116;

const LVCF_FMT: UINT = 0x0001;
const LVCF_WIDTH: UINT = 0x0002;
const LVCF_TEXT: UINT = 0x0004;
const LVCF_SUBITEM: UINT = 0x0008;
const LVCFMT_LEFT: i32 = 0;
const LVIF_TEXT: UINT = 0x0001;
const LVIS_FOCUSED: UINT = 0x0001;
const LVIS_SELECTED: UINT = 0x0002;
const LVIS_STATEIMAGEMASK: UINT = 0xF000;

const NM_FIRST: UINT = 0;
const LVN_FIRST: UINT = 0u32.wrapping_sub(100);
const NM_DBLCLK: UINT = NM_FIRST.wrapping_sub(3);
const LVN_ITEMCHANGED: UINT = LVN_FIRST.wrapping_sub(1);

const ICC_LISTVIEW_CLASSES: DWORD = 0x0000_0001;
const SPI_GETNONCLIENTMETRICS: UINT = 0x0029;
const LOGPIXELSX: i32 = 88;
const COLOR_BTNFACE: i32 = 15;
const TRANSPARENT: i32 = 1;

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;

const VK_TAB: u16 = 9;
const VK_ESCAPE: u16 = 27;
const VK_SHIFT: i32 = 0x10;

const MB_OK: UINT = 0x0000_0000;
const MB_ICONINFORMATION: UINT = 0x0000_0040;
const MB_ICONWARNING: UINT = 0x0000_0030;
const MB_YESNO: UINT = 0x0000_0004;
const MB_ICONQUESTION: UINT = 0x0000_0020;
const IDYES: i32 = 6;

// 控件 ID
const ID_VIEW: i32 = 1001;
const ID_EXIT: i32 = 1002;
const ID_ALL: i32 = 1003;
const ID_NONE: i32 = 1004;
const ID_UPGRADE: i32 = 1005;
const ID_BACK: i32 = 1006;
const ID_TITLE: i32 = 1007;
const ID_SUBTITLE: i32 = 1008;
const ID_STATUS: i32 = 1009;
const ID_LIST: i32 = 1010;

// ---------------------------------------------------------------- Win32 导入

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> ATOM;
    fn CreateWindowExW(
        ex_style: DWORD,
        class_name: LPCWSTR,
        window_name: LPCWSTR,
        style: DWORD,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        menu: HMENU,
        instance: HINSTANCE,
        param: *mut c_void,
    ) -> HWND;
    fn DefWindowProcW(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    fn UpdateWindow(hwnd: HWND) -> BOOL;
    fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: UINT, max: UINT) -> BOOL;
    fn TranslateMessage(msg: *const MSG) -> BOOL;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn PostQuitMessage(code: i32);
    fn DestroyWindow(hwnd: HWND) -> BOOL;
    fn SendMessageW(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT;
    fn MessageBoxW(hwnd: HWND, text: LPCWSTR, caption: LPCWSTR, flags: UINT) -> i32;
    fn SetWindowTextW(hwnd: HWND, text: LPCWSTR) -> BOOL;
    fn EnableWindow(hwnd: HWND, enable: BOOL) -> BOOL;
    fn IsWindowVisible(hwnd: HWND) -> BOOL;
    fn IsWindowEnabled(hwnd: HWND) -> BOOL;
    fn SetFocus(hwnd: HWND) -> HWND;
    fn GetFocus() -> HWND;
    fn GetKeyState(key: i32) -> i16;
    fn SetTimer(hwnd: HWND, id: UINT_PTR, elapse: UINT, proc_: TIMERPROC) -> UINT_PTR;
    fn KillTimer(hwnd: HWND, id: UINT_PTR) -> BOOL;
    fn AdjustWindowRectEx(rect: *mut RECT, style: DWORD, menu: BOOL, ex_style: DWORD) -> BOOL;
    fn LoadCursorW(instance: HINSTANCE, name: LPCWSTR) -> HCURSOR;
    fn LoadIconW(instance: HINSTANCE, name: LPCWSTR) -> HICON;
    fn SetProcessDPIAware() -> BOOL;
    fn SystemParametersInfoW(action: UINT, param: UINT, data: *mut c_void, win_ini: UINT) -> BOOL;
    fn GetDC(hwnd: HWND) -> HDC;
    fn ReleaseDC(hwnd: HWND, dc: HDC) -> i32;
    fn GetSysColor(index: i32) -> DWORD;
    fn GetSysColorBrush(index: i32) -> HBRUSH;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateFontIndirectW(log_font: *const LOGFONTW) -> HFONT;
    fn DeleteObject(obj: HGDIOBJ) -> BOOL;
    fn GetDeviceCaps(dc: HDC, index: i32) -> i32;
    fn SetBkMode(dc: HDC, mode: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: LPCWSTR) -> HMODULE;
    fn MultiByteToWideChar(
        code_page: UINT,
        flags: DWORD,
        s: *const u8,
        s_len: i32,
        out: LPWSTR,
        out_len: i32,
    ) -> i32;
    fn AllocConsole() -> BOOL;
    fn GetConsoleWindow() -> HWND;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn InitCommonControlsEx(icc: *const INITCOMMONCONTROLEX) -> BOOL;
}

// 修正：comctl32 的初始化结构体名
#[repr(C)]
struct INITCOMMONCONTROLEX {
    dw_size: DWORD,
    dw_icc: DWORD,
}

// ---------------------------------------------------------------- 小工具

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_text(hwnd: HWND, text: &str) {
    if hwnd.is_null() {
        return;
    }
    let t = wide(text);
    unsafe {
        SetWindowTextW(hwnd, t.as_ptr());
    }
}

fn show(hwnd: HWND, visible: bool) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
    }
}

fn enable(hwnd: HWND, on: bool) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        EnableWindow(hwnd, on as BOOL);
    }
}

fn message_box(owner: HWND, text: &str, flags: UINT) {
    let _ = message_box_ex(owner, text, flags);
}

fn message_box_ex(owner: HWND, text: &str, flags: UINT) -> i32 {
    let t = wide(text);
    let c = wide("winget 选择性更新");
    unsafe { MessageBoxW(owner, t.as_ptr(), c.as_ptr(), flags) }
}

// ---------------------------------------------------------------- 数据模型

#[derive(Clone)]
struct Item {
    name: String,
    id: String,
    current: String,
    available: String,
}

// ---------------------------------------------------------------- 表格解析
// winget 的表格是“按显示宽度左对齐 + 单空格分隔 + 末列不填充”，
// 表头各列的起始显示列 = 该列数据起始显示列（与语言无关）。

fn char_width(c: char) -> usize {
    let c = c as u32;
    if c == 0 {
        return 0;
    }
    // 组合符
    if (0x0300..=0x036F).contains(&c)
        || (0x1AB0..=0x1AFF).contains(&c)
        || (0x1DC0..=0x1DFF).contains(&c)
        || (0x20D0..=0x20FF).contains(&c)
        || (0xFE20..=0xFE2F).contains(&c)
    {
        return 0;
    }
    if (0x1100..=0x115F).contains(&c)
        || (0x2E80..=0x303E).contains(&c)
        || (0x3041..=0x33FF).contains(&c)
        || (0x3400..=0x4DBF).contains(&c)
        || (0x4E00..=0x9FFF).contains(&c)
        || (0xA000..=0xA4CF).contains(&c)
        || (0xA960..=0xA97F).contains(&c)
        || (0xAC00..=0xD7A3).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
        || (0xFE10..=0xFE19).contains(&c)
        || (0xFE30..=0xFE6F).contains(&c)
        || (0xFF00..=0xFF60).contains(&c)
        || (0xFFE0..=0xFFE6).contains(&c)
        || (0x1F300..=0x1F64F).contains(&c)
        || (0x1F900..=0x1F9FF).contains(&c)
        || (0x20000..=0x3FFFD).contains(&c)
    {
        2
    } else {
        1
    }
}

fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// 表头按空白切成若干“词”，每个词的起始显示列就是该列起点。
fn header_starts(header: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut pos = 0usize;
    let mut prev_space = true;
    for ch in header.chars() {
        let cw = char_width(ch);
        if ch == ' ' || ch == '\t' {
            prev_space = true;
        } else {
            if prev_space {
                starts.push(pos);
            }
            prev_space = false;
        }
        pos += cw;
    }
    // 第一列起点固定为 0（防止行首空格导致偏移）
    if let Some(first) = starts.first_mut() {
        *first = 0;
    }
    starts
}

/// 取 [start, end) 显示列范围内的字符并 trim。
fn slice_range(line: &str, start: usize, end: usize) -> String {
    let mut out = String::new();
    let mut pos = 0usize;
    for ch in line.chars() {
        let cw = char_width(ch);
        if pos >= start && pos < end && ch != '\r' {
            out.push(ch);
        }
        pos += cw;
        if pos >= end {
            break;
        }
    }
    out.trim().to_string()
}

fn row_to_item(cells: &[String]) -> Option<Item> {
    if cells.len() < 4 {
        return None;
    }
    let name = cells[0].trim();
    let id = cells[1].trim();
    let current = cells[2].trim();
    let available = cells[3].trim();
    if name.is_empty() || id.is_empty() || current.is_empty() || available.is_empty() {
        return None;
    }
    if id.len() > 128 || id.contains(char::is_whitespace) {
        return None;
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
    {
        return None;
    }
    Some(Item {
        name: name.to_string(),
        id: id.to_string(),
        current: current.to_string(),
        available: available.to_string(),
    })
}

/// 解析 `winget upgrade` 的表格输出。
/// 返回 None 表示没有找到表格；Some(vec) 表示找到了表格（可能为空）。
fn parse_upgrade_table(text: &str) -> Option<Vec<Item>> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    // 1) 找到全由 '-'/空格组成的分隔线
    let sep_idx = lines.iter().position(|l| {
        let t = l.trim();
        !t.is_empty() && t.len() >= 20 && t.contains('-') && t.chars().all(|c| c == '-' || c == ' ')
    })?;
    if sep_idx == 0 {
        return None;
    }
    let table_width = display_width(lines[sep_idx].trim_end());

    // 2) 表头词的起始位置 = 各列起点
    let header = lines[sep_idx - 1];
    let starts = header_starts(header);
    if starts.len() < 4 {
        return None;
    }

    // 3) 逐行取单元格
    let mut items = Vec::new();
    for line in lines.iter().skip(sep_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if items.is_empty() {
                continue;
            }
            break;
        }
        // 数据行至少要有表格那么宽（末列较短时允许少量差距）
        if table_width > 16 && display_width(line) + 8 < table_width {
            break;
        }
        let mut cells = Vec::with_capacity(starts.len());
        for (i, start) in starts.iter().enumerate() {
            let end = starts.get(i + 1).copied().unwrap_or(usize::MAX);
            cells.push(slice_range(line, *start, end));
        }
        match row_to_item(&cells) {
            Some(item) => items.push(item),
            None => break,
        }
    }
    Some(items)
}

// ---------------------------------------------------------------- winget 调用


fn decode_output(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    unsafe {
        let len = MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut buf = vec![0u16; len as usize];
        MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, buf.as_mut_ptr(), len);
        String::from_utf16_lossy(&buf)
    }
}

/// winget 的 AppX 激活在某些系统上需要调用方拥有控制台。
/// 本程序是 GUI 子系统，这里分配一个隐藏控制台供子进程继承（随后立即隐藏窗口）。
fn ensure_hidden_console() {
    unsafe {
        if GetConsoleWindow().is_null() {
            if AllocConsole() != 0 {
                let wnd = GetConsoleWindow();
                if !wnd.is_null() {
                    ShowWindow(wnd, SW_HIDE);
                }
            }
        }
    }
}

/// 调用 winget：优先直接启动；若直接启动失败且没有任何输出（部分系统上
/// AppX 激活会异常），再用 cmd.exe 作中转并重定向到临时文件。
fn winget_run(args: &[&str], timeout: Option<std::time::Duration>) -> Result<WingetOutput, String> {
    let force_cmd = std::env::var_os("WGUP_FORCE_CMD").is_some();
    let first = if force_cmd {
        winget_run_via_cmd(args, timeout)
    } else {
        winget_run_direct(args, timeout)
    };
    let need_retry = match &first {
        Ok(out) => !out.success && out.stdout.trim().is_empty() && out.stderr.trim().is_empty(),
        Err(_) => true,
    };
    if !need_retry {
        return first;
    }
    let second = if force_cmd {
        winget_run_direct(args, timeout)
    } else {
        winget_run_via_cmd(args, timeout)
    };
    second.or(first)
}

fn winget_run_direct(
    args: &[&str],
    timeout: Option<std::time::Duration>,
) -> Result<WingetOutput, String> {
    use std::io::Read;
    let mut last_err = String::new();
    for exe in ["winget.exe", "winget"] {
        let spawned = Command::new(exe)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => {
                c
            }
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        let mut out_pipe = child.stdout.take().expect("stdout pipe");
        let mut err_pipe = child.stderr.take().expect("stderr pipe");
        // 并发读取两根管道，避免 winget 输出过多时阻塞
        let (out_tx, out_rx) = channel();
        let (err_tx, err_rx) = channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = out_pipe.read_to_end(&mut buf);
            let _ = out_tx.send(buf);
        });
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = err_pipe.read_to_end(&mut buf);
            let _ = err_tx.send(buf);
        });
        let mut timed_out = false;
        let exit = match timeout {
            Some(limit) => {
                let deadline = std::time::Instant::now() + limit;
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break Some(status),
                        Ok(None) => {
                            if std::time::Instant::now() >= deadline {
                                let _ = child.kill();
                                let _ = child.wait();
                                timed_out = true;
                                break None;
                            }
                            thread::sleep(std::time::Duration::from_millis(50));
                        }
                        Err(e) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            last_err = e.to_string();
                            break None;
                        }
                    }
                }
            }
            None => match child.wait() {
                Ok(status) => Some(status),
                Err(e) => {
                    last_err = e.to_string();
                    None
                }
            },
        };
        // 有界等待：某些环境下 winget 由 AppX 激活器托管，管道可能长期不关闭
        let read_wait = std::time::Duration::from_secs(3);
        let raw_out = out_rx.recv_timeout(read_wait).unwrap_or_default();
        let raw_err = err_rx.recv_timeout(read_wait).unwrap_or_default();
        if timed_out {
            let secs = timeout.map(|d| d.as_secs()).unwrap_or(0);
            return Err(format!(
                "winget 响应超时（{secs} 秒）。\n可能是网络或软件源问题，请稍后重试。"
            ));
        }
        let status = match exit {
            Some(s) => s,
            None => continue,
        };
        return Ok(WingetOutput {
            success: status.success(),
            code: status.code().unwrap_or(-1),
            stdout: decode_output(&raw_out),
            stderr: decode_output(&raw_err),
        });
    }
    Err(format!(
        "无法启动 winget（{last_err}）。\n请确认系统已安装“应用安装程序”(App Installer)。"
    ))
}

/// 通过 cmd.exe 调用 winget（输出重定向到临时文件），用于规避某些系统上
/// 直接 CreateProcess 启动 App Execution Alias 时的激活异常。
fn winget_run_via_cmd(
    args: &[&str],
    timeout: Option<std::time::Duration>,
) -> Result<WingetOutput, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir();
    let out_path = dir.join(format!("winget-picker-{}-{}.out", std::process::id(), stamp));
    let err_path = dir.join(format!("winget-picker-{}-{}.err", std::process::id(), stamp));
    let mut cmdline = String::from("winget ");
    for arg in args {
        cmdline.push_str(arg);
        cmdline.push(' ');
    }
    cmdline.push_str(&format!(
        "> \"{}\" 2>&1",
        out_path.display()
    ));
    let spawned = Command::new("cmd.exe")
        .args(["/d", "/c", &cmdline])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(c) => {
            c
        }
        Err(e) => return Err(format!("无法启动 cmd.exe（{e}）")),
    };
    let mut timed_out = false;
    let status = match timeout {
        Some(limit) => {
            let deadline = std::time::Instant::now() + limit;
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Some(status),
                    Ok(None) => {
                        if std::time::Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            timed_out = true;
                            break None;
                        }
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(_) => break None,
                }
            }
        }
        None => child.wait().ok(),
    };
    let out_bytes = std::fs::read(&out_path).unwrap_or_default();
    let err_bytes = std::fs::read(&err_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&err_path);
    if timed_out {
        let secs = timeout.map(|d| d.as_secs()).unwrap_or(0);
        return Err(format!(
            "winget 响应超时（{secs} 秒）。\n可能是网络或软件源问题，请稍后重试。"
        ));
    }
    let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    Ok(WingetOutput {
        success: code == 0,
        code,
        stdout: decode_output(&out_bytes),
        stderr: decode_output(&err_bytes),
    })
}

struct WingetOutput {
    success: bool,
    code: i32,
    stdout: String,
    stderr: String,
}

fn sanitize_line(s: &str) -> String {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if c == '\t' {
                ' '
            } else if u < 0x20 || u == 0x7f {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn tail_lines(text: &str, n: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let s = sanitize_line(line);
        if s.is_empty() {
            continue;
        }
        out.push(s);
        if out.len() > n {
            out.remove(0);
        }
    }
    truncate_chars(&out.join("\n"), 700)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

fn code_str(code: i32) -> String {
    if code < 0 {
        format!("0x{:08X}", code as u32)
    } else {
        format!("退出码 {code}")
    }
}

/// 获取可更新的软件列表；返回 Err 表示 winget 执行失败。
fn fetch_upgrades() -> Result<Vec<Item>, String> {
    let out = winget_run(
        &[
            "upgrade",
            "--include-unknown",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
        Some(std::time::Duration::from_secs(120)),
    )?;
    if let Some(items) = parse_upgrade_table(&out.stdout) {
        return Ok(items);
    }
    if out.success {
        // 没有可更新的软件时 winget 只打印一句提示（无表格），退出码为 0
        return Ok(Vec::new());
    }
    let mut detail = tail_lines(&out.stdout, 5);
    if !out.stderr.trim().is_empty() {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&tail_lines(&out.stderr, 4));
    }
    Err(format!(
        "winget 返回错误（{}）。\n{}",
        code_str(out.code),
        detail.trim()
    ))
}

/// 升级单个软件包。
fn upgrade_package(id: &str) -> Result<(), String> {
    let out = winget_run(
        &[
            "upgrade",
            "--id",
            id,
            "--exact",
            "--include-unknown",
            "--accept-source-agreements",
            "--accept-package-agreements",
            "--disable-interactivity",
        ],
        None,
    )?;
    if out.success {
        return Ok(());
    }
    let mut text = out.stdout;
    if !out.stderr.trim().is_empty() {
        text.push('\n');
        text.push_str(&out.stderr);
    }
    let detail = tail_lines(&text, 8);
    if detail.is_empty() {
        Err(format!("winget 失败（{}）", code_str(out.code)))
    } else {
        Err(detail)
    }
}

// ---------------------------------------------------------------- 后台任务消息

enum TaskMsg {
    Fetched(Result<Vec<Item>, String>),
    ProgressStart {
        index: usize,
        name: String,
        n: usize,
        total: usize,
    },
    ProgressResult {
        index: usize,
        ok: bool,
    },
    UpgradeDone {
        ok: usize,
        fail: usize,
        failures: Vec<(String, String)>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Init,
    Loading,
    List,
    Upgrading,
}

enum CloseDecision {
    Allow,
    Confirm(String),
}

// ---------------------------------------------------------------- 应用状态

thread_local! {
    static APP: RefCell<Option<Box<App>>> = const { RefCell::new(None) };
}

fn with_app(f: impl FnOnce(&mut App)) {
    let _ = APP.try_with(|cell| {
        // 控件在 SendMessage 期间可能同步回调窗口过程（例如 LVM_* 触发的
        // LVN_ITEMCHANGED）。这类重入不修改状态，直接跳过以避免 RefCell 冲突。
        if let Ok(mut borrow) = cell.try_borrow_mut() {
            if let Some(app) = borrow.as_mut() {
                f(app);
            }
        }
    });
}

struct App {
    hwnd: HWND,
    title: HWND,
    subtitle: HWND,
    btn_view: HWND,
    btn_exit: HWND,
    status: HWND,
    list: HWND,
    btn_all: HWND,
    btn_none: HWND,
    btn_upgrade: HWND,
    btn_back: HWND,
    phase: Phase,
    items: Vec<Item>,
    tx: Sender<TaskMsg>,
    rx: Receiver<TaskMsg>,
    timer_on: bool,
    total: usize,
    finished: usize,
    ok: usize,
    fail: usize,
}

unsafe fn make_child(
    parent: HWND,
    instance: HINSTANCE,
    class: &[u16],
    text: &str,
    style: DWORD,
    ex_style: DWORD,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
    font: HFONT,
) -> HWND {
    let t = wide(text);
    let hwnd = CreateWindowExW(
        ex_style,
        class.as_ptr(),
        t.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        id as usize as HMENU,
        instance,
        std::ptr::null_mut(),
    );
    if !hwnd.is_null() && !font.is_null() {
        SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1);
    }
    hwnd
}

impl App {
    unsafe fn new(hwnd: HWND) -> Box<App> {
        // DPI
        let mut dpi: i32 = 96;
        let screen_dc = GetDC(std::ptr::null_mut());
        if !screen_dc.is_null() {
            let d = GetDeviceCaps(screen_dc, LOGPIXELSX);
            if d > 0 {
                dpi = d;
            }
            ReleaseDC(std::ptr::null_mut(), screen_dc);
        }
        let s = |v: i32| -> i32 { (v as i64 * dpi as i64 / 96) as i32 };

        // 字体（使用系统消息字体，自动跟随 DPI）
        let mut ncm = NONCLIENTMETRICS::default();
        ncm.cb_size = std::mem::size_of::<NONCLIENTMETRICS>() as UINT;
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cb_size,
            &mut ncm as *mut _ as *mut c_void,
            0,
        );
        let font = CreateFontIndirectW(&ncm.lf_message_font);
        let mut title_lf = ncm.lf_message_font;
        title_lf.lf_height = -s(24);
        title_lf.lf_weight = 600;
        let title_font = CreateFontIndirectW(&title_lf);

        let instance = GetModuleHandleW(std::ptr::null());
        let class_static = wide("STATIC");
        let class_button = wide("BUTTON");
        let class_list = wide("SysListView32");

        // 初始界面
        let title = make_child(
            hwnd, instance, &class_static, "winget 选择性更新",
            SS_CENTER | SS_NOPREFIX, 0, 0, s(38), s(820), s(40), ID_TITLE, title_font,
        );
        let subtitle = make_child(
            hwnd, instance, &class_static, "先查看可更新的软件，勾选后再只升级你选择的部分",
            SS_CENTER | SS_NOPREFIX, 0, 0, s(92), s(820), s(26), ID_SUBTITLE, font,
        );
        let btn_view = make_child(
            hwnd, instance, &class_button, "查看升级",
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(250), s(170), s(150), s(44), ID_VIEW, font,
        );
        let btn_exit = make_child(
            hwnd, instance, &class_button, "退出",
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(420), s(170), s(150), s(44), ID_EXIT, font,
        );

        // 清单界面
        let status = make_child(
            hwnd, instance, &class_static, "",
            SS_LEFT | SS_NOPREFIX, 0, s(16), s(12), s(788), s(24), ID_STATUS, font,
        );
        let list = make_child(
            hwnd, instance, &class_list, "",
            LVS_REPORT | LVS_SHOWSELALWAYS | WS_TABSTOP,
            WS_EX_CLIENTEDGE, s(16), s(42), s(788), s(422), ID_LIST, font,
        );
        if !list.is_null() {
            let ex_style = (LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES | LVS_EX_CHECKBOXES | LVS_EX_DOUBLEBUFFER) as LPARAM;
            SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, ex_style);
            let columns: [(&str, i32); 5] = [
                ("名称", 270),
                ("当前版本", 105),
                ("可用版本", 105),
                ("ID", 195),
                ("结果", 65),
            ];
            for (i, (text, width)) in columns.iter().enumerate() {
                let t = wide(text);
                let mut col = LVCOLUMNW::default();
                col.mask = LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM;
                col.fmt = LVCFMT_LEFT;
                col.cx = s(*width);
                col.psz_text = t.as_ptr();
                col.cch_text = 0;
                col.i_sub_item = i as i32;
                SendMessageW(
                    list,
                    LVM_INSERTCOLUMNW,
                    i as WPARAM,
                    &mut col as *mut _ as LPARAM,
                );
            }
            let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
            if !header.is_null() {
                SendMessageW(header, WM_SETFONT, font as WPARAM, 1);
            }
        }
        let btn_all = make_child(
            hwnd, instance, &class_button, "全选",
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(16), s(472), s(90), s(32), ID_ALL, font,
        );
        let btn_none = make_child(
            hwnd, instance, &class_button, "全不选",
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(114), s(472), s(90), s(32), ID_NONE, font,
        );
        let btn_upgrade = make_child(
            hwnd, instance, &class_button, "升级选中 (0)",
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(212), s(472), s(150), s(32), ID_UPGRADE, font,
        );
        let btn_back = make_child(
            hwnd, instance, &class_button, "返回",
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(714), s(472), s(90), s(32), ID_BACK, font,
        );

        let (tx, rx) = channel();

        let mut app = Box::new(App {
            hwnd,
            title,
            subtitle,
            btn_view,
            btn_exit,
            status,
            list,
            btn_all,
            btn_none,
            btn_upgrade,
            btn_back,
            phase: Phase::Init,
            items: Vec::new(),
            tx,
            rx,
            timer_on: false,
            total: 0,
            finished: 0,
            ok: 0,
            fail: 0,
        });
        app.apply_phase();
        app
    }

    fn apply_phase(&mut self) {
        let phase = self.phase;
        let init = phase == Phase::Init;
        let list_mode = phase == Phase::List;
        show(self.title, init);
        show(self.subtitle, init);
        show(self.btn_view, init);
        show(self.btn_exit, init);
        show(self.status, !init);
        show(self.list, !init);
        show(self.btn_all, !init);
        show(self.btn_none, !init);
        show(self.btn_upgrade, !init);
        show(self.btn_back, !init);
        enable(self.list, list_mode);
        enable(self.btn_all, list_mode);
        enable(self.btn_none, list_mode);
        enable(self.btn_back, list_mode);
        enable(self.btn_upgrade, false);
        if list_mode {
            self.refresh_upgrade_button();
            unsafe {
                SetFocus(self.list);
            }
        } else if init {
            unsafe {
                SetFocus(self.btn_view);
            }
        }
    }

    fn checked_indices(&self) -> Vec<usize> {
        let count = unsafe { SendMessageW(self.list, LVM_GETITEMCOUNT, 0, 0) } as usize;
        let mut out = Vec::new();
        for i in 0..count {
            let state = unsafe {
                SendMessageW(self.list, LVM_GETITEMSTATE, i, LVIS_STATEIMAGEMASK as LPARAM)
            } as u32;
            if (state >> 12) == 2 {
                out.push(i);
            }
        }
        out
    }

    fn set_checked(&self, index: usize, on: bool) {
        let mut item = LVITEMW::default();
        item.state_mask = LVIS_STATEIMAGEMASK;
        item.state = if on { 2 << 12 } else { 1 << 12 };
        unsafe {
            SendMessageW(
                self.list,
                LVM_SETITEMSTATE,
                index,
                &mut item as *mut _ as LPARAM,
            );
        }
    }

    fn refresh_upgrade_button(&mut self) {
        let n = self.checked_indices().len();
        set_text(self.btn_upgrade, &format!("升级选中 ({n})"));
        enable(self.btn_upgrade, self.phase == Phase::List && n > 0);
    }

    fn set_status(&self, text: &str) {
        set_text(self.status, text);
    }

    fn set_sub_text(&self, row: usize, col: i32, text: &str) {
        let t = wide(text);
        let mut item = LVITEMW::default();
        item.i_sub_item = col;
        item.psz_text = t.as_ptr() as *mut u16;
        item.cch_text = -1;
        unsafe {
            SendMessageW(
                self.list,
                LVM_SETITEMTEXTW,
                row,
                &mut item as *mut _ as LPARAM,
            );
        }
    }

    fn set_result(&self, row: usize, text: &str) {
        self.set_sub_text(row, 4, text);
    }

    fn insert_row(&self, index: usize) {
        let item = &self.items[index];
        let name = wide(&item.name);
        let mut row = LVITEMW::default();
        row.mask = LVIF_TEXT;
        row.i_item = index as i32;
        row.i_sub_item = 0;
        row.psz_text = name.as_ptr() as *mut u16;
        row.cch_text = -1;
        unsafe {
            SendMessageW(
                self.list,
                LVM_INSERTITEMW,
                index,
                &mut row as *mut _ as LPARAM,
            );
        }
        self.set_sub_text(index, 1, &item.current);
        self.set_sub_text(index, 2, &item.available);
        self.set_sub_text(index, 3, &item.id);
    }

    fn start_timer(&mut self) {
        if !self.timer_on {
            unsafe {
                SetTimer(self.hwnd, 1, 80, None);
            }
            self.timer_on = true;
        }
    }

    fn stop_timer(&mut self) {
        if self.timer_on {
            unsafe {
                KillTimer(self.hwnd, 1);
            }
            self.timer_on = false;
        }
    }

    fn take_messages(&mut self) -> Vec<TaskMsg> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(m) => out.push(m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    fn begin_fetch(&mut self) {
        self.items.clear();
        self.phase = Phase::Loading;
        unsafe {
            SendMessageW(self.list, LVM_DELETEALLITEMS, 0, 0);
        }
        self.set_status("正在获取可更新的软件列表，请稍候…");
        self.apply_phase();
        self.start_timer();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = fetch_upgrades();
            let _ = tx.send(TaskMsg::Fetched(result));
        });
    }

    fn fill_list(&mut self, mut items: Vec<Item>) {
        items.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        self.items = items;
        for i in 0..self.items.len() {
            self.insert_row(i);
        }
        let n = self.items.len();
        self.phase = Phase::List;
        self.set_status(&format!("共 {n} 个可更新软件。勾选后点击“升级选中”。"));
        self.apply_phase();
        if n > 0 {
            // 让第一行获得键盘焦点，使空格键可直接勾选
            let mut item = LVITEMW::default();
            item.state_mask = LVIS_FOCUSED | LVIS_SELECTED;
            item.state = LVIS_FOCUSED | LVIS_SELECTED;
            unsafe {
                SendMessageW(self.list, LVM_SETITEMSTATE, 0, &mut item as *mut _ as LPARAM);
            }
        }
    }

    fn begin_upgrade(&mut self, indices: Vec<usize>) {
        let total = indices.len();
        let mut selected: Vec<(usize, Item)> = Vec::with_capacity(total);
        for &i in &indices {
            if let Some(item) = self.items.get(i) {
                selected.push((i, item.clone()));
                self.set_result(i, "等待中");
            }
        }
        self.phase = Phase::Upgrading;
        self.total = total;
        self.finished = 0;
        self.ok = 0;
        self.fail = 0;
        self.set_status(&format!("准备升级 {total} 个软件…"));
        self.apply_phase();
        self.start_timer();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let total = selected.len();
            let mut ok = 0usize;
            let mut failures: Vec<(String, String)> = Vec::new();
            for (n, (index, item)) in selected.into_iter().enumerate() {
                if tx
                    .send(TaskMsg::ProgressStart {
                        index,
                        name: item.name.clone(),
                        n: n + 1,
                        total,
                    })
                    .is_err()
                {
                    return;
                }
                match upgrade_package(&item.id) {
                    Ok(()) => {
                        ok += 1;
                        let _ = tx.send(TaskMsg::ProgressResult { index, ok: true });
                    }
                    Err(e) => {
                        failures.push((item.name.clone(), e));
                        let _ = tx.send(TaskMsg::ProgressResult { index, ok: false });
                    }
                }
            }
            let _ = tx.send(TaskMsg::UpgradeDone {
                ok,
                fail: failures.len(),
                failures,
            });
        });
    }

    fn reset_to_init(&mut self) {
        self.stop_timer();
        self.phase = Phase::Init;
        self.items.clear();
        unsafe {
            SendMessageW(self.list, LVM_DELETEALLITEMS, 0, 0);
        }
        self.total = 0;
        self.finished = 0;
        self.ok = 0;
        self.fail = 0;
        self.set_status("");
        self.apply_phase();
    }

    fn handle(&mut self, msg: TaskMsg) -> Option<String> {
        match msg {
            TaskMsg::Fetched(result) => {
                self.stop_timer();
                match result {
                    Err(e) => {
                        self.reset_to_init();
                        Some(format!("获取可更新软件列表失败。\n\n{e}"))
                    }
                    Ok(items) if items.is_empty() => {
                        self.reset_to_init();
                        Some("没有检测到可更新的软件，系统当前已是最新状态。".to_string())
                    }
                    Ok(items) => {
                        self.fill_list(items);
                        None
                    }
                }
            }
            TaskMsg::ProgressStart {
                index,
                name,
                n,
                total,
            } => {
                self.set_status(&format!("正在升级（{n}/{total}）：{name}"));
                self.set_result(index, "升级中…");
                None
            }
            TaskMsg::ProgressResult { index, ok } => {
                self.set_result(index, if ok { "成功" } else { "失败" });
                self.finished += 1;
                if ok {
                    self.ok += 1;
                } else {
                    self.fail += 1;
                }
                self.set_status(&format!(
                    "已处理 {}/{}（成功 {}，失败 {}）",
                    self.finished, self.total, self.ok, self.fail
                ));
                None
            }
            TaskMsg::UpgradeDone { ok, fail, failures } => {
                self.stop_timer();
                let mut text = if fail == 0 {
                    format!("本轮升级完成：{ok} 个软件全部升级成功。")
                } else {
                    format!("本轮升级结束：成功 {ok} 个，失败 {fail} 个。")
                };
                if !failures.is_empty() {
                    text.push_str("\n\n以下软件未能升级：");
                    for (i, (name, detail)) in failures.iter().take(5).enumerate() {
                        text.push_str(&format!("\n{}. {}", i + 1, name));
                        let d = detail.replace('\n', " ");
                        let d = d.trim();
                        if !d.is_empty() {
                            text.push_str(&format!("\n    {}", truncate_chars(d, 140)));
                        }
                    }
                    if failures.len() > 5 {
                        text.push_str(&format!("\n… 另有 {} 个失败。", failures.len() - 5));
                    }
                }
                self.reset_to_init();
                Some(text)
            }
        }
    }

    fn on_command(&mut self, id: i32) -> Option<String> {
        match id {
            ID_VIEW => {
                if self.phase == Phase::Init {
                    self.begin_fetch();
                }
                None
            }
            ID_EXIT => {
                unsafe {
                    DestroyWindow(self.hwnd);
                }
                None
            }
            ID_ALL => {
                if self.phase == Phase::List {
                    for i in 0..self.items.len() {
                        self.set_checked(i, true);
                    }
                    self.refresh_upgrade_button();
                }
                None
            }
            ID_NONE => {
                if self.phase == Phase::List {
                    for i in 0..self.items.len() {
                        self.set_checked(i, false);
                    }
                    self.refresh_upgrade_button();
                }
                None
            }
            ID_BACK => {
                if self.phase == Phase::List {
                    self.reset_to_init();
                }
                None
            }
            ID_UPGRADE => {
                if self.phase != Phase::List {
                    return None;
                }
                let indices = self.checked_indices();
                if indices.is_empty() {
                    return Some("请先勾选需要升级的软件。".to_string());
                }
                self.begin_upgrade(indices);
                None
            }
            _ => None,
        }
    }

    unsafe fn on_notify(&mut self, lparam: LPARAM) {
        if lparam == 0 {
            return;
        }
        let hdr = &*(lparam as *const NMHDR);
        if hdr.id_from != ID_LIST as usize {
            return;
        }
        if self.phase != Phase::List {
            return;
        }
        let code = hdr.code as i32;
        if hdr.code == LVN_ITEMCHANGED {
            self.refresh_upgrade_button();
        } else if hdr.code == NM_DBLCLK {
            let nmlv = &*(lparam as *const NMLISTVIEW);
            if nmlv.i_item >= 0 {
                let index = nmlv.i_item as usize;
                if index < self.items.len() {
                    let state = SendMessageW(
                        self.list,
                        LVM_GETITEMSTATE,
                        index,
                        LVIS_STATEIMAGEMASK as LPARAM,
                    ) as u32;
                    let on = (state >> 12) != 2;
                    self.set_checked(index, on);
                    self.refresh_upgrade_button();
                }
            }
        }
        let _ = code;
    }

    fn on_close(&mut self) -> CloseDecision {
        if self.phase == Phase::Upgrading {
            self.stop_timer();
            CloseDecision::Confirm(
                "升级仍在后台进行中。\n\n确定要退出界面吗？（已启动的 winget 进程可能继续运行）"
                    .to_string(),
            )
        } else {
            CloseDecision::Allow
        }
    }

    fn resume_after_dialog(&mut self) {
        if self.phase == Phase::Loading || self.phase == Phase::Upgrading {
            self.start_timer();
        }
    }

    fn escape_pressed(&mut self) {
        if self.phase == Phase::List {
            self.reset_to_init();
        }
    }

    fn focus_next(prev: bool) {
        with_app(|app| {
            let candidates: Vec<HWND> = match app.phase {
                Phase::Init => vec![app.btn_view, app.btn_exit],
                Phase::List => vec![
                    app.list,
                    app.btn_all,
                    app.btn_none,
                    app.btn_upgrade,
                    app.btn_back,
                ],
                _ => Vec::new(),
            };
            let enabled: Vec<HWND> = candidates
                .into_iter()
                .filter(|&h| unsafe {
                    !h.is_null() && IsWindowVisible(h) != 0 && IsWindowEnabled(h) != 0
                })
                .collect();
            if enabled.is_empty() {
                return;
            }
            let focused = unsafe { GetFocus() };
            let current = enabled.iter().position(|&h| h == focused);
            let next = match (prev, current) {
                (false, None) => 0,
                (false, Some(i)) => (i + 1) % enabled.len(),
                (true, None) => enabled.len() - 1,
                (true, Some(0)) => enabled.len() - 1,
                (true, Some(i)) => i - 1,
            };
            unsafe {
                SetFocus(enabled[next]);
            }
        });
    }
}

// ---------------------------------------------------------------- 窗口过程

unsafe extern "system" fn wndproc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let mut popup: Option<String> = None;
            with_app(|app| {
                popup = app.on_command(id);
            });
            if let Some(text) = popup {
                message_box(hwnd, &text, MB_OK | MB_ICONINFORMATION);
            }
            0
        }
        WM_NOTIFY => {
            with_app(|app| app.on_notify(lparam));
            0
        }
        WM_TIMER => {
            let mut messages = Vec::new();
            with_app(|app| messages = app.take_messages());
            for m in messages {
                let mut popup = None;
                with_app(|app| popup = app.handle(m));
                if let Some(text) = popup {
                    message_box(hwnd, &text, MB_OK | MB_ICONINFORMATION);
                }
            }
            0
        }
        WM_CLOSE => {
            let mut decision = CloseDecision::Allow;
            with_app(|app| decision = app.on_close());
            match decision {
                CloseDecision::Allow => {
                    DestroyWindow(hwnd);
                }
                CloseDecision::Confirm(text) => {
                    if message_box_ex(hwnd, &text, MB_YESNO | MB_ICONQUESTION) == IDYES {
                        DestroyWindow(hwnd);
                    } else {
                        with_app(|app| app.resume_after_dialog());
                    }
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        WM_CTLCOLORSTATIC => {
            let hdc = wparam as HDC;
            SetBkMode(hdc, TRANSPARENT);
            let _ = GetSysColor(COLOR_BTNFACE);
            GetSysColorBrush(COLOR_BTNFACE) as LRESULT
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------- 入口

/// 诊断入口：`winget-picker.exe --check-winget <输出文件>`
/// 直接调用一次 winget 并写出解析结果，用于自检 winget 调用是否可用。
fn check_winget_mode() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) != Some("--check-winget") {
        return false;
    }
    let out_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "winget-check.txt".to_string());
    let text = match fetch_upgrades() {
        Ok(items) => {
            let mut s = format!("OK count={}\n", items.len());
            for item in items.iter().take(8) {
                s.push_str(&format!(
                    "{} | {} | {} | {}\n",
                    item.name, item.current, item.available, item.id
                ));
            }
            s
        }
        Err(e) => format!("ERR {e}\n"),
    };
    let _ = std::fs::write(&out_path, text);
    true
}

fn main() {
    ensure_hidden_console();
    #[cfg(debug_assertions)]
    if check_winget_mode() {
        return;
    }
    unsafe {
        let _ = SetProcessDPIAware();
        let icc = INITCOMMONCONTROLEX {
            dw_size: std::mem::size_of::<INITCOMMONCONTROLEX>() as DWORD,
            dw_icc: ICC_LISTVIEW_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("WingetPickerWnd");
        let class_ptr = class_name.as_ptr();

        let wc = WNDCLASSEXW {
            cb_size: std::mem::size_of::<WNDCLASSEXW>() as UINT,
            style: 0,
            lpfn_wnd_proc: Some(wndproc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: instance,
            h_icon: LoadIconW(std::ptr::null_mut(), 32512usize as LPCWSTR),
            h_cursor: LoadCursorW(std::ptr::null_mut(), 32512usize as LPCWSTR),
            hbr_background: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_ptr,
            h_icon_sm: LoadIconW(std::ptr::null_mut(), 32512usize as LPCWSTR),
        };
        if RegisterClassExW(&wc) == 0 {
            return;
        }
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;

        // 依据系统 DPI 计算窗口尺寸
        let mut dpi: i32 = 96;
        let screen_dc = GetDC(std::ptr::null_mut());
        if !screen_dc.is_null() {
            let d = GetDeviceCaps(screen_dc, LOGPIXELSX);
            if d > 0 {
                dpi = d;
            }
            ReleaseDC(std::ptr::null_mut(), screen_dc);
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 820 * dpi / 96,
            bottom: 520 * dpi / 96,
        };
        AdjustWindowRectEx(&mut rect, style, 0, 0);
        let title = wide("winget 选择性更新");
        let hwnd = CreateWindowExW(
            0,
            class_ptr,
            title.as_ptr(),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            message_box(std::ptr::null_mut(), "创建窗口失败。", MB_OK | MB_ICONWARNING);
            return;
        }

        let app = App::new(hwnd);
        APP.with(|cell| {
            *cell.borrow_mut() = Some(app);
        });

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if ret <= 0 {
                break;
            }
            if msg.message == WM_KEYDOWN {
                let vk = msg.w_param as u16;
                if vk == VK_TAB {
                    App::focus_next(GetKeyState(VK_SHIFT) < 0);
                    continue;
                }
                if vk == VK_ESCAPE {
                    with_app(|app| app.escape_pressed());
                    continue;
                }
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

// ---------------------------------------------------------------- 测试

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ZH: &str = include_str!("../testdata/winget_upgrade_zh.txt");

    fn build_table(rows: &[[&str; 5]], footer: Option<&str>) -> String {
        let headers = ["Name", "Id", "Version", "Available", "Source"];
        let mut widths = [0usize; 5];
        for (i, h) in headers.iter().enumerate() {
            widths[i] = display_width(h);
        }
        for row in rows {
            for i in 0..5 {
                widths[i] = widths[i].max(display_width(row[i]));
            }
        }
        let pad = |s: &str, w: usize| -> String {
            let mut out = s.to_string();
            for _ in display_width(s)..w {
                out.push(' ');
            }
            out
        };
        let mut lines = Vec::new();
        let mut header_line = String::new();
        for i in 0..5 {
            header_line.push_str(&pad(headers[i], widths[i]));
            if i < 4 {
                header_line.push(' ');
            }
        }
        lines.push(header_line);
        lines.push("-".repeat(widths.iter().map(|w| w + 1).sum::<usize>() - 1));
        for row in rows {
            let mut line = String::new();
            for i in 0..5 {
                line.push_str(&pad(row[i], widths[i]));
                if i < 4 {
                    line.push(' ');
                }
            }
            lines.push(line);
        }
        if let Some(f) = footer {
            lines.push(f.to_string());
        }
        lines.join("\r\n") + "\r\n"
    }

    #[test]
    fn display_width_cjk() {
        assert_eq!(display_width("微信"), 4);
        assert_eq!(display_width("Git"), 3);
        assert_eq!(display_width("v2.1.6 x64 汉化版"), 17);
    }

    #[test]
    fn parse_real_winget_output() {
        let items = parse_upgrade_table(SAMPLE_ZH).expect("应能找到表格");
        assert_eq!(items.len(), 47, "真实样本应有 47 个可升级条目");
        let find = |id: &str| items.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("缺少 {id}"));

        let wechat = find("Tencent.WeChat.Universal");
        assert_eq!(wechat.name, "微信");
        assert_eq!(wechat.current, "4.1.15.56");
        assert_eq!(wechat.available, "4.2.0.52");

        let openal = find("CreativeTechnology.OpenAL");
        assert_eq!(openal.current, "Unknown");
        assert_eq!(openal.available, "1.1");

        let foobar = find("PeterPawlowski.foobar2000");
        assert_eq!(foobar.current, "v2.1.6 x64 汉化版");

        let vcredist = find("Microsoft.VCRedist.2015+.x64");
        assert_eq!(
            vcredist.name,
            "Microsoft Visual C++ v14 Redistributable (x64) - 14.51.36247"
        );

        let dotnet = find("Microsoft.DotNet.Runtime.3_1");
        assert_eq!(dotnet.current, "< 3.1.25");

        for item in &items {
            assert!(!item.name.contains("升级可用"), "不应把结尾统计行当成软件");
            assert!(!item.id.contains(' '));
            assert!(!item.id.is_empty());
        }
    }

    #[test]
    fn parse_synthetic_table_with_wide_and_msstore() {
        let rows = [
            ["Git", "Git.Git", "2.43.0", "2.44.0", "winget"],
            ["微信", "Tencent.WeChat.Universal", "4.1.15.56", "4.2.0.52", "winget"],
            [
                "foobar2000 汉化版",
                "PeterPawlowski.foobar2000",
                "v2.1.6 x64 汉化版",
                "2.26.2",
                "msstore",
            ],
            [
                "Microsoft Build of OpenJDK  25.0.4.1+1 (x64)",
                "Microsoft.OpenJDK.25",
                "< 3.1.25",
                "26.13.1",
                "winget",
            ],
        ];
        let table = build_table(&rows, Some("4 upgrades available."));
        let items = parse_upgrade_table(&table).expect("应能找到表格");
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].name, "Git");
        assert_eq!(items[1].name, "微信");
        assert_eq!(items[1].available, "4.2.0.52");
        assert_eq!(items[2].current, "v2.1.6 x64 汉化版");
        assert_eq!(items[3].name, "Microsoft Build of OpenJDK  25.0.4.1+1 (x64)");
        assert_eq!(items[3].current, "< 3.1.25");
    }

    #[test]
    fn parse_english_headers_with_short_version_width() {
        // Version 列宽恰好等于表头宽度时，表头“Version”与下一列之间只有 1 个空格
        let rows = [
            ["A", "Vendor.A", "1.2.3.4", "1.2.3.5", "winget"],
            ["B", "Vendor.B", "1.0", "1.1", "winget"],
        ];
        let table = build_table(&rows, None);
        let items = parse_upgrade_table(&table).expect("应能找到表格");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].current, "1.2.3.4");
        assert_eq!(items[1].available, "1.1");
    }

    #[test]
    fn parse_empty_table_and_no_table() {
        let table = build_table(&[], None);
        let items = parse_upgrade_table(&table).expect("应能找到表格");
        assert!(items.is_empty());

        let message = "找不到与输入条件匹配的已安装程序包。\r\n";
        assert!(parse_upgrade_table(message).is_none());
    }
}
