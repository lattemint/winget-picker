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

mod config;
mod lang;
mod theme;

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
type HIMAGELIST = *mut c_void;
type HBITMAP = *mut c_void;
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
struct SIZE {
    cx: i32,
    cy: i32,
}
zero_default!(SIZE);

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

#[repr(C)]
#[derive(Clone, Copy)]
struct NMCUSTOMDRAW {
    hdr: NMHDR,
    draw_stage: UINT,
    hdc: HDC,
    rc: RECT,
    item_spec: usize,
    u_item: UINT,
    item_l_param: LPARAM,
}
zero_default!(NMCUSTOMDRAW);

#[repr(C)]
#[derive(Clone, Copy)]
struct NMLVCUSTOMDRAW {
    nmcd: NMCUSTOMDRAW,
    clr_text: u32,
    clr_text_bk: u32,
    i_sub_item: i32,
}
zero_default!(NMLVCUSTOMDRAW);

#[repr(C)]
#[derive(Clone, Copy)]
struct PAINTSTRUCT {
    hdc: HDC,
    f_erase: BOOL,
    rc_paint: RECT,
    f_restore: BOOL,
    f_inc_update: BOOL,
    rgb_reserved: [u8; 32],
}
zero_default!(PAINTSTRUCT);

#[repr(C)]
#[derive(Clone, Copy)]
struct TRACKMOUSEEVENT {
    cb_size: DWORD,
    dw_flags: DWORD,
    hwnd_track: HWND,
    dw_hover_time: DWORD,
}
zero_default!(TRACKMOUSEEVENT);

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFOHEADER {
    bi_size: DWORD,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: DWORD,
    bi_size_image: DWORD,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: DWORD,
    bi_clr_important: DWORD,
}
zero_default!(BITMAPINFOHEADER);

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [u32; 1],
}
zero_default!(BITMAPINFO);

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
const SS_RIGHT: DWORD = 0x0000_0002;
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
const LVM_ENSUREVISIBLE: UINT = LVM_FIRST + 19;
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
const NM_CUSTOMDRAW: UINT = NM_FIRST.wrapping_sub(12);

// ListView 自定义绘制（用于行内进度着色）
const CDDS_PREPAINT: UINT = 0x0000_0001;
const CDDS_ITEMPREPAINT: UINT = 0x0001_0001;
const CDDS_SUBITEM: UINT = 0x0002_0000;
const CDRF_DODEFAULT: LRESULT = 0;
const CDRF_NEWFONT: LRESULT = 0x2;
const CDRF_NOTIFYITEMDRAW: LRESULT = 0x20;
const CDRF_NOTIFYSUBITEMDRAW: LRESULT = 0x20;

const ICC_LISTVIEW_CLASSES: DWORD = 0x0000_0001;
const ICC_PROGRESS_CLASS: DWORD = 0x0000_0020;

// 进度条
const PBM_SETRANGE32: UINT = 0x0400 + 6;
const PBM_SETPOS: UINT = 0x0400 + 2;
const PBS_SMOOTH: DWORD = 0x0001;
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

/// 列表列标题由 i18n 提供（`lang::t().columns`）。

const MB_OK: UINT = 0x0000_0000;
const MB_ICONINFORMATION: UINT = 0x0000_0040;
const MB_ICONWARNING: UINT = 0x0000_0030;
const MB_YESNO: UINT = 0x0000_0004;
const MB_ICONQUESTION: UINT = 0x0000_0020;
const MB_DEFBUTTON2: UINT = 0x0000_0100;
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
const ID_PROGRESS: i32 = 1011;
const ID_THEME_LABEL: i32 = 1012;
const ID_THEME_BASE: i32 = 1020; // 1020..=1024：5 个主题按钮
const ID_LANG_LABEL: i32 = 1013;
const ID_LANG_BASE: i32 = 1030; // 1030..=1032：跟随系统 / 中文 / English
const ID_ABOUT: i32 = 1014;
const ID_ABOUT_CLOSE: i32 = 1040;
const ID_SILENT_LABEL: i32 = 1015;
const ID_SILENT_OFF: i32 = 1016;
const ID_SILENT_ON: i32 = 1017;

// Spinner
const NULL_PEN: i32 = 8;

/// 关于窗里的项目地址
const REPO_URL: &str = "https://github.com/lattemint/winget-picker/";

// 关于窗
const WM_SETCURSOR: UINT = 0x0020;
const WM_NCDESTROY: UINT = 0x0082;
const WS_POPUP: DWORD = 0x8000_0000;
const GWLP_USERDATA: i32 = -21;
const DT_WORDBREAK: UINT = 0x0010;
const DT_LEFT: UINT = 0x0000;
const IMAGE_ICON: UINT = 1;
const DI_NORMAL: UINT = 0x0003;
const IDC_HAND: LPCWSTR = 32649usize as LPCWSTR;

// 自绘控件相关
const WM_PAINT: UINT = 0x000F;
const WM_ERASEBKGND: UINT = 0x0014;
const WM_MOUSEMOVE: UINT = 0x0200;
const WM_LBUTTONDOWN: UINT = 0x0201;
const WM_LBUTTONUP: UINT = 0x0202;
const WM_CAPTURECHANGED: UINT = 0x0215;
const WM_MOUSELEAVE: UINT = 0x02A3;
const WM_ENABLE: UINT = 0x000A;
const WM_SETFOCUS: UINT = 0x0007;
const WM_KILLFOCUS: UINT = 0x0008;
const WM_GETFONT: UINT = 0x0031;
const WM_SETTEXT: UINT = 0x000C;
const BN_CLICKED: UINT = 0;
const VK_RETURN: u16 = 13;
const VK_SPACE: u16 = 32;
const DT_CENTER: UINT = 0x0001;
const DT_VCENTER: UINT = 0x0004;
const DT_SINGLELINE: UINT = 0x0020;
const TME_LEAVE: u32 = 0x0000_0002;
const GWLP_ID: i32 = -12;
const LVS_NOCOLUMNHEADER: DWORD = 0x4000;

// ListView 颜色/表头/网格线
const LVM_SETBKCOLOR: UINT = LVM_FIRST + 1;
const LVM_SETIMAGELIST: UINT = LVM_FIRST + 2;
const LVM_GETITEMRECT: UINT = LVM_FIRST + 14;
const LVM_SETTEXTCOLOR: UINT = LVM_FIRST + 36;
const LVM_SETTEXTBKCOLOR: UINT = LVM_FIRST + 38;
const LVM_GETTOPINDEX: UINT = LVM_FIRST + 39;
const LVSIL_STATE: WPARAM = 2;
const CDDS_POSTPAINT: UINT = 0x0000_0002;

// 经典进度条上色
const PBM_SETBARCOLOR: UINT = 0x0400 + 9;
const PBM_SETBKCOLOR: UINT = 0x2001;

// 自绘按钮状态位（存于窗口类额外内存 offset 0）
const BTN_HOT: isize = 1;
const BTN_PRESSED: isize = 2;
const BTN_SELECTED: isize = 4;
const BTN_FONT_OFFSET: i32 = 8;

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
    fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: BOOL) -> BOOL;
    fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    fn GetWindowTextW(hwnd: HWND, text: LPWSTR, max: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: HWND, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: HWND, index: i32, value: isize) -> isize;
    fn GetParent(hwnd: HWND) -> HWND;
    fn GetCapture() -> HWND;
    fn SetCapture(hwnd: HWND) -> HWND;
    fn ReleaseCapture() -> BOOL;
    fn DrawTextW(hdc: HDC, text: LPCWSTR, len: i32, rect: *mut RECT, flags: UINT) -> i32;
    fn BeginPaint(hwnd: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(hwnd: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    fn DrawFocusRect(hdc: HDC, rect: *const RECT) -> BOOL;
    fn TrackMouseEvent(event: *mut TRACKMOUSEEVENT) -> BOOL;
    fn GetDlgCtrlID(hwnd: HWND) -> i32;
    fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    fn IsWindow(hwnd: HWND) -> BOOL;
    fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    fn SetCursor(cursor: HCURSOR) -> HCURSOR;
    fn GetCursorPos(point: *mut POINT) -> BOOL;
    fn ScreenToClient(hwnd: HWND, point: *mut POINT) -> BOOL;
    fn LoadImageW(
        instance: HINSTANCE,
        name: LPCWSTR,
        kind: UINT,
        cx: i32,
        cy: i32,
        flags: UINT,
    ) -> *mut c_void;
    fn DrawIconEx(
        hdc: HDC,
        x: i32,
        y: i32,
        icon: HICON,
        cx: i32,
        cy: i32,
        istep: UINT,
        brush: HBRUSH,
        flags: UINT,
    ) -> BOOL;
    fn DestroyIcon(icon: HICON) -> BOOL;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateFontIndirectW(log_font: *const LOGFONTW) -> HFONT;
    fn DeleteObject(obj: HGDIOBJ) -> BOOL;
    fn GetDeviceCaps(dc: HDC, index: i32) -> i32;
    fn SetBkMode(dc: HDC, mode: i32) -> i32;
    fn SetTextColor(dc: HDC, color: u32) -> u32;
    fn SetBkColor(dc: HDC, color: u32) -> u32;
    fn CreateSolidBrush(color: u32) -> HBRUSH;
    fn FillRect(dc: HDC, rect: *const RECT, brush: HBRUSH) -> i32;
    fn GetStockObject(index: i32) -> HGDIOBJ;
    fn SelectObject(dc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
    fn CreateDIBSection(
        dc: HDC,
        bmi: *const BITMAPINFO,
        usage: UINT,
        bits: *mut *mut c_void,
        section: *mut c_void,
        offset: DWORD,
    ) -> HBITMAP;
    fn GetTextExtentPoint32W(
        dc: HDC,
        text: *const u16,
        len: i32,
        size: *mut SIZE,
    ) -> BOOL;
    fn Ellipse(dc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: HWND,
        operation: LPCWSTR,
        file: LPCWSTR,
        parameters: LPCWSTR,
        directory: LPCWSTR,
        show_cmd: i32,
    ) -> HINSTANCE;
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
    fn LoadLibraryW(name: LPCWSTR) -> HMODULE;
    fn GetProcAddress(module: HMODULE, name: *const u8) -> *mut c_void;
    fn GetUserDefaultUILanguage() -> u16;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn InitCommonControlsEx(icc: *const INITCOMMONCONTROLEX) -> BOOL;
    fn ImageList_Create(cx: i32, cy: i32, flags: UINT, initial: i32, grow: i32) -> HIMAGELIST;
    fn ImageList_Add(list: HIMAGELIST, bitmap: HBITMAP, mask: HBITMAP) -> i32;
    fn ImageList_Destroy(list: HIMAGELIST) -> BOOL;
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

/// COLORREF：0x00BBGGRR
fn rgb(r: u32, g: u32, b: u32) -> u32 {
    (r & 0xFF) | ((g & 0xFF) << 8) | ((b & 0xFF) << 16)
}

/// 系统 UI 语言是否为中文（用于“跟随系统”）。
fn system_ui_is_chinese() -> bool {
    unsafe { (GetUserDefaultUILanguage() & 0x03FF) == 0x0004 }
}

fn message_box(owner: HWND, text: &str, flags: UINT) {
    let _ = message_box_ex(owner, text, flags);
}

fn message_box_ex(owner: HWND, text: &str, flags: UINT) -> i32 {
    let t = wide(text);
    let c = wide(lang::t().app_name);
    unsafe { MessageBoxW(owner, t.as_ptr(), c.as_ptr(), flags) }
}

// ---------------------------------------------------------------- 主题绘制辅助

static mut UI_BRUSH_BG: HBRUSH = std::ptr::null_mut();
static mut UI_FONT: HFONT = std::ptr::null_mut();

fn ui_brush_bg() -> HBRUSH {
    unsafe { UI_BRUSH_BG }
}

/// 动态加载 DLL 函数（DWM 暗色标题栏 / 经典控件主题）。
unsafe fn dyn_load(dll: &str, func: &str) -> *mut c_void {
    let module = LoadLibraryW(wide(dll).as_ptr());
    if module.is_null() {
        return std::ptr::null_mut();
    }
    let name = format!("{}\0", func);
    GetProcAddress(module, name.as_ptr() as *const u8)
}

unsafe fn set_dark_titlebar(hwnd: HWND, dark: bool) {
    type Fn = unsafe extern "system" fn(HWND, u32, *const c_void, u32) -> i32;
    let addr = dyn_load("dwmapi.dll", "DwmSetWindowAttribute");
    if addr.is_null() {
        return;
    }
    let f: Fn = std::mem::transmute(addr);
    let value: i32 = dark as i32;
    // 20 = DWMWA_USE_IMMERSIVE_DARK_MODE（Win10 2004+）；19 为旧版本属性
    f(hwnd, 20, &value as *const i32 as *const c_void, 4);
    f(hwnd, 19, &value as *const i32 as *const c_void, 4);
}

unsafe fn set_classic_theme(hwnd: HWND) {
    type Fn = unsafe extern "system" fn(HWND, *const u16, *const u16) -> i32;
    let addr = dyn_load("uxtheme.dll", "SetWindowTheme");
    if addr.is_null() {
        return;
    }
    let f: Fn = std::mem::transmute(addr);
    let empty = wide("");
    f(hwnd, empty.as_ptr(), empty.as_ptr());
}

fn color_luma(color: u32) -> f32 {
    let r = (color & 0xFF) as f32;
    let g = ((color >> 8) & 0xFF) as f32;
    let b = ((color >> 16) & 0xFF) as f32;
    0.299 * r + 0.587 * g + 0.114 * b
}

/// 生成 16x16 自定义勾选框图片：0=空白，1=未勾选，2=勾选。
unsafe fn make_state_bitmap(th: &theme::Theme, kind: u8) -> HBITMAP {
    let screen = GetDC(std::ptr::null_mut());
    let bmi = BITMAPINFO {
        bmi_header: BITMAPINFOHEADER {
            bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as DWORD,
            bi_width: 16,
            bi_height: -16,
            bi_planes: 1,
            bi_bit_count: 32,
            bi_compression: 0,
            bi_size_image: 16 * 16 * 4,
            bi_x_pels_per_meter: 0,
            bi_y_pels_per_meter: 0,
            bi_clr_used: 0,
            bi_clr_important: 0,
        },
        bmi_colors: [0],
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let dib = CreateDIBSection(screen, &bmi, 0, &mut bits, std::ptr::null_mut(), 0);
    ReleaseDC(std::ptr::null_mut(), screen);
    if dib.is_null() || bits.is_null() {
        return std::ptr::null_mut();
    }
    let px = bits as *mut u32;
    let bg = 0xFF00_0000u32 | th.list_bg;
    for i in 0..256 {
        px.add(i).write(bg);
    }
    match kind {
        1 => {
            let border = 0xFF00_0000u32 | th.muted;
            for i in 2..=13 {
                px.add(2 * 16 + i).write(border);
                px.add(13 * 16 + i).write(border);
                px.add(i * 16 + 2).write(border);
                px.add(i * 16 + 13).write(border);
            }
        }
        2 => {
            let fill = 0xFF00_0000u32 | th.accent;
            for y in 2..=13 {
                for x in 2..=13 {
                    px.add(y * 16 + x).write(fill);
                }
            }
            let check = if color_luma(th.accent) > 150.0 {
                0xFF00_0000u32 | th.bg
            } else {
                0xFFFF_FFFFu32
            };
            let pts = [
                (4usize, 8usize),
                (5, 9),
                (6, 10),
                (7, 9),
                (8, 8),
                (9, 7),
                (10, 6),
                (11, 5),
            ];
            for &(x, y) in pts.iter() {
                px.add(y * 16 + x).write(check);
                if y + 1 < 16 {
                    px.add((y + 1) * 16 + x).write(check);
                }
            }
        }
        _ => {}
    }
    dib
}

unsafe fn make_state_images(th: &theme::Theme) -> HIMAGELIST {
    let list = ImageList_Create(16, 16, 0x0000_0020, 3, 1); // ILC_COLOR32
    if list.is_null() {
        return list;
    }
    for kind in [0u8, 1, 2] {
        let bmp = make_state_bitmap(th, kind);
        if !bmp.is_null() {
            ImageList_Add(list, bmp, std::ptr::null_mut());
            DeleteObject(bmp as HGDIOBJ);
        }
    }
    list
}

unsafe fn button_notify(hwnd: HWND) {
    let parent = GetParent(hwnd);
    if parent.is_null() {
        return;
    }
    let id = GetDlgCtrlID(hwnd) as usize;
    SendMessageW(parent, WM_COMMAND, id, hwnd as LPARAM);
}

unsafe fn button_paint(hwnd: HWND) {
    let th = *theme::active();
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);

    let state = GetWindowLongPtrW(hwnd, 0);
    let enabled = IsWindowEnabled(hwnd) != 0;
    let selected = state & BTN_SELECTED != 0;
    let pressed = state & BTN_PRESSED != 0;
    let hot = state & BTN_HOT != 0;

    let bg = if !enabled {
        th.btn_bg
    } else if selected {
        th.accent_soft
    } else if pressed {
        th.btn_pressed
    } else if hot {
        th.btn_hover
    } else {
        th.btn_bg
    };
    let fg = if !enabled {
        th.btn_disabled_text
    } else if selected {
        th.accent
    } else {
        th.btn_text
    };
    let border = if selected { th.accent } else { th.border };

    let brush = CreateSolidBrush(bg);
    FillRect(hdc, &rc, brush);
    DeleteObject(brush as HGDIOBJ);

    let border_brush = CreateSolidBrush(border);
    let lines = [
        RECT { left: 0, top: 0, right: rc.right, bottom: 1 },
        RECT { left: 0, top: rc.bottom - 1, right: rc.right, bottom: rc.bottom },
        RECT { left: 0, top: 0, right: 1, bottom: rc.bottom },
        RECT { left: rc.right - 1, top: 0, right: rc.right, bottom: rc.bottom },
    ];
    for line in lines.iter() {
        FillRect(hdc, line, border_brush);
    }
    DeleteObject(border_brush as HGDIOBJ);

    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, fg);
    let font = GetWindowLongPtrW(hwnd, BTN_FONT_OFFSET) as HFONT;
    if !font.is_null() {
        SelectObject(hdc, font as HGDIOBJ);
    }
    let mut buf = [0u16; 256];
    let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), 256);
    if n > 0 {
        let mut tr = rc;
        tr.left += 6;
        tr.right -= 6;
        DrawTextW(hdc, buf.as_ptr(), n, &mut tr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }
    if GetFocus() == hwnd {
        let mut fr = rc;
        fr.left += 3;
        fr.top += 3;
        fr.right -= 3;
        fr.bottom -= 3;
        DrawFocusRect(hdc, &fr);
    }
    EndPaint(hwnd, &ps);
}

unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut state = GetWindowLongPtrW(hwnd, 0);
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            button_paint(hwnd);
            0
        }
        WM_SETFONT => {
            SetWindowLongPtrW(hwnd, BTN_FONT_OFFSET, wparam as isize);
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_GETFONT => GetWindowLongPtrW(hwnd, BTN_FONT_OFFSET),
        WM_SETTEXT => {
            let r = DefWindowProcW(hwnd, msg, wparam, lparam);
            InvalidateRect(hwnd, std::ptr::null(), 1);
            r
        }
        WM_ENABLE => {
            InvalidateRect(hwnd, std::ptr::null(), 1);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEMOVE => {
            if state & BTN_HOT == 0 {
                state |= BTN_HOT;
                SetWindowLongPtrW(hwnd, 0, state);
                let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
                tme.cb_size = std::mem::size_of::<TRACKMOUSEEVENT>() as DWORD;
                tme.dw_flags = TME_LEAVE;
                tme.hwnd_track = hwnd;
                TrackMouseEvent(&mut tme);
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            0
        }
        WM_MOUSELEAVE => {
            state &= !BTN_HOT;
            SetWindowLongPtrW(hwnd, 0, state);
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_LBUTTONDOWN => {
            state |= BTN_PRESSED;
            SetWindowLongPtrW(hwnd, 0, state);
            SetCapture(hwnd);
            SetFocus(hwnd);
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_LBUTTONUP => {
            let was_pressed = state & BTN_PRESSED != 0;
            state &= !BTN_PRESSED;
            SetWindowLongPtrW(hwnd, 0, state);
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            if was_pressed {
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                if x >= 0 && y >= 0 && x < rc.right && y < rc.bottom {
                    button_notify(hwnd);
                }
            }
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_CAPTURECHANGED => {
            state &= !BTN_PRESSED;
            SetWindowLongPtrW(hwnd, 0, state);
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_KEYDOWN => {
            let vk = wparam as u16;
            if vk == VK_SPACE || vk == VK_RETURN {
                button_notify(hwnd);
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            InvalidateRect(hwnd, std::ptr::null(), 1);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn set_button_selected(hwnd: HWND, on: bool) {
    if hwnd.is_null() {
        return;
    }
    let mut state = GetWindowLongPtrW(hwnd, 0);
    if on {
        state |= BTN_SELECTED;
    } else {
        state &= !BTN_SELECTED;
    }
    SetWindowLongPtrW(hwnd, 0, state);
    InvalidateRect(hwnd, std::ptr::null(), 1);
}

// ---------------------------------------------------------------- 关于窗

struct AboutState {
    font: HFONT,
    title_font: HFONT,
    dpi: i32,
    icon: HICON,
    close: HWND,
    link_rect: RECT,
}

unsafe fn about_paint(hwnd: HWND) {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AboutState;
    if state_ptr.is_null() {
        return;
    }
    let state = &mut *state_ptr;
    let th = *theme::active();
    let t = lang::t();
    let s = |v: i32| -> i32 { (v as i64 * state.dpi as i64 / 96) as i32 };

    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);
    let bg = CreateSolidBrush(th.bg);
    FillRect(hdc, &rc, bg);
    DeleteObject(bg as HGDIOBJ);
    SetBkMode(hdc, TRANSPARENT);

    // logo
    if !state.icon.is_null() {
        DrawIconEx(hdc, s(24), s(24), state.icon, s(96), s(96), 0, std::ptr::null_mut(), DI_NORMAL);
    }

    // 名称
    SelectObject(hdc, state.title_font as HGDIOBJ);
    SetTextColor(hdc, th.text);
    let name_text = wide(t.app_name);
    let mut name_rect = RECT {
        left: s(136),
        top: s(24),
        right: rc.right - s(24),
        bottom: s(58),
    };
    DrawTextW(hdc, name_text.as_ptr(), -1, &mut name_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    // 版本
    SelectObject(hdc, state.font as HGDIOBJ);
    SetTextColor(hdc, th.muted);
    let version = wide(&format!("v{}", env!("CARGO_PKG_VERSION")));
    let mut ver_rect = RECT {
        left: s(140),
        top: s(60),
        right: rc.right - s(24),
        bottom: s(82),
    };
    DrawTextW(hdc, version.as_ptr(), -1, &mut ver_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    // 简介（整行宽度）
    SetTextColor(hdc, th.text);
    let desc = wide(t.about_desc);
    let mut desc_rect = RECT {
        left: s(24),
        top: s(124),
        right: rc.right - s(24),
        bottom: s(166),
    };
    DrawTextW(hdc, desc.as_ptr(), -1, &mut desc_rect, DT_LEFT | DT_WORDBREAK);

    // 技术副标
    SetTextColor(hdc, th.muted);
    let tech = wide(t.about_tech);
    let mut tech_rect = RECT {
        left: s(24),
        top: s(170),
        right: rc.right - s(24),
        bottom: s(194),
    };
    DrawTextW(hdc, tech.as_ptr(), -1, &mut tech_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    // 项目链接
    let label = wide(t.about_link_label);
    let mut label_rect = RECT {
        left: s(24),
        top: s(196),
        right: rc.right - s(24),
        bottom: s(222),
    };
    DrawTextW(hdc, label.as_ptr(), -1, &mut label_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    let mut label_size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(hdc, label.as_ptr(), label.len() as i32 - 1, &mut label_size);
    let link_x = s(24) + label_size.cx + s(2);
    let url = wide(REPO_URL);
    SetTextColor(hdc, th.accent);
    let mut url_rect = RECT {
        left: link_x,
        top: s(196),
        right: rc.right - s(24),
        bottom: s(222),
    };
    DrawTextW(hdc, url.as_ptr(), -1, &mut url_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    let mut url_size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(hdc, url.as_ptr(), url.len() as i32 - 1, &mut url_size);
    let underline = RECT {
        left: link_x,
        top: s(220),
        right: link_x + url_size.cx,
        bottom: s(221),
    };
    let underline_brush = CreateSolidBrush(th.accent);
    FillRect(hdc, &underline, underline_brush);
    DeleteObject(underline_brush as HGDIOBJ);
    state.link_rect = RECT {
        left: link_x,
        top: s(196),
        right: link_x + url_size.cx,
        bottom: s(226),
    };

    // 数据说明 / 许可
    SetTextColor(hdc, th.muted);
    let data = wide(t.about_data);
    let mut data_rect = RECT {
        left: s(24),
        top: s(230),
        right: rc.right - s(24),
        bottom: s(284),
    };
    DrawTextW(hdc, data.as_ptr(), -1, &mut data_rect, DT_LEFT | DT_WORDBREAK);
    let license = wide(t.about_license);
    let mut license_rect = RECT {
        left: s(24),
        top: s(286),
        right: rc.right - s(24),
        bottom: s(310),
    };
    DrawTextW(hdc, license.as_ptr(), -1, &mut license_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    EndPaint(hwnd, &ps);
}

unsafe extern "system" fn about_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => {
            let hdc = wparam as HDC;
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let th = *theme::active();
            let brush = CreateSolidBrush(th.bg);
            FillRect(hdc, &rc, brush);
            DeleteObject(brush as HGDIOBJ);
            1
        }
        WM_PAINT => {
            about_paint(hwnd);
            0
        }
        WM_SETCURSOR => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AboutState;
            if !state_ptr.is_null() {
                let mut point = POINT { x: 0, y: 0 };
                if GetCursorPos(&mut point) != 0 {
                    ScreenToClient(hwnd, &mut point);
                    let r = (*state_ptr).link_rect;
                    if point.x >= r.left
                        && point.x < r.right
                        && point.y >= r.top
                        && point.y < r.bottom
                    {
                        SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_HAND));
                        return 1;
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_LBUTTONUP => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AboutState;
            if !state_ptr.is_null() {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let r = (*state_ptr).link_rect;
                if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                    let op = wide("open");
                    let url = wide(REPO_URL);
                    ShellExecuteW(
                        hwnd,
                        op.as_ptr(),
                        url.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        1,
                    );
                }
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            if id == ID_ABOUT_CLOSE {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_NCDESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AboutState;
            if !state_ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let state = Box::from_raw(state_ptr);
                if !state.icon.is_null() {
                    DestroyIcon(state.icon);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_about(owner: HWND, font: HFONT, title_font: HFONT, dpi: i32) {
    let instance = GetModuleHandleW(std::ptr::null());
    let class = wide("WPAbout");
    let s = |v: i32| -> i32 { (v as i64 * dpi as i64 / 96) as i32 };
    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: s(540),
        bottom: s(360),
    };
    AdjustWindowRectEx(&mut frame, style, 0, 0);
    let win_w = frame.right - frame.left;
    let win_h = frame.bottom - frame.top;
    let mut owner_rect: RECT = std::mem::zeroed();
    GetWindowRect(owner, &mut owner_rect);
    let x = owner_rect.left + ((owner_rect.right - owner_rect.left) - win_w) / 2;
    let y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - win_h) / 2;

    let state = Box::into_raw(Box::new(AboutState {
        font,
        title_font,
        dpi,
        icon: LoadImageW(instance, 1usize as LPCWSTR, IMAGE_ICON, s(96), s(96), 0) as HICON,
        close: std::ptr::null_mut(),
        link_rect: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
    }));
    let title = wide(lang::t().app_name);
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        title.as_ptr(),
        style,
        x,
        y,
        win_w,
        win_h,
        owner,
        std::ptr::null_mut(),
        instance,
        std::ptr::null_mut(),
    );
    if hwnd.is_null() {
        let state = Box::from_raw(state);
        if !state.icon.is_null() {
            DestroyIcon(state.icon);
        }
        return;
    }
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
    set_dark_titlebar(hwnd, theme::active().dark);

    let close_class = wide("WPButton");
    let close_text = wide(lang::t().about_close);
    let close = CreateWindowExW(
        0,
        close_class.as_ptr(),
        close_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        s(540) - s(24) - s(100),
        s(360) - s(24) - s(30),
        s(100),
        s(30),
        hwnd,
        ID_ABOUT_CLOSE as usize as HMENU,
        instance,
        std::ptr::null_mut(),
    );
    if !close.is_null() {
        SendMessageW(close, WM_SETFONT, font as WPARAM, 1);
        (*state).close = close;
    }

    EnableWindow(owner, 0);
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    if !close.is_null() {
        SetFocus(close);
    }

    let mut msg: MSG = std::mem::zeroed();
    loop {
        let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
        if ret <= 0 {
            break;
        }
        if msg.hwnd == hwnd && msg.message == WM_KEYDOWN && msg.w_param as u16 == VK_ESCAPE {
            DestroyWindow(hwnd);
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
        if IsWindow(hwnd) == 0 {
            break;
        }
    }
    if IsWindow(owner) != 0 {
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
    }
}

// ---------------------------------------------------------------- 加载转圈

fn mix_color(a: u32, b: u32, t: f64) -> u32 {
    let ch = |shift: u32| -> u32 {
        let av = ((a >> shift) & 0xFF) as f64;
        let bv = ((b >> shift) & 0xFF) as f64;
        (((av + (bv - av) * t).round() as i64).clamp(0, 255) as u32) << shift
    };
    ch(0) | ch(8) | ch(16)
}

/// 在主窗口上直接绘制转圈（与表头一致，不额外创建子窗口）。
unsafe fn draw_spinner(hdc: HDC, rect: &RECT, frame: i64) {
    let th = *theme::active();
    let n = 8i64;
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let cx = rect.left as f64 + w as f64 / 2.0;
    let cy = rect.top as f64 + h as f64 / 2.0;
    let radius = (w.min(h) as f64) / 2.0 - 1.5;
    let dot = (w / 5).max(3);
    for i in 0..n {
        let phase = ((i + frame) % n + n) % n;
        let factor = 0.25 + 0.75 * (phase as f64) / ((n - 1) as f64);
        let color = mix_color(th.bg, th.accent, factor);
        let angle = (i as f64) * std::f64::consts::TAU / (n as f64);
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        let left = (x.round() as i32) - dot / 2;
        let top = (y.round() as i32) - dot / 2;
        let dot_rect = RECT {
            left,
            top,
            right: left + dot,
            bottom: top + dot,
        };
        let brush = CreateSolidBrush(color);
        FillRect(hdc, &dot_rect, brush);
        DeleteObject(brush as HGDIOBJ);
    }
}

unsafe fn register_control_classes(instance: HINSTANCE) {
    let class = wide("WPButton");
    let wc = WNDCLASSEXW {
        cb_size: std::mem::size_of::<WNDCLASSEXW>() as UINT,
        style: 0,
        lpfn_wnd_proc: Some(button_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 16,
        h_instance: instance,
        h_icon: std::ptr::null_mut(),
        h_cursor: LoadCursorW(std::ptr::null_mut(), 32512usize as LPCWSTR),
        hbr_background: std::ptr::null_mut(),
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class.as_ptr(),
        h_icon_sm: std::ptr::null_mut(),
    };
    RegisterClassExW(&wc);

    let about_class = wide("WPAbout");
    let about_icon = LoadIconW(instance, 1usize as LPCWSTR);
    let wc_about = WNDCLASSEXW {
        cb_size: std::mem::size_of::<WNDCLASSEXW>() as UINT,
        style: 0,
        lpfn_wnd_proc: Some(about_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: instance,
        h_icon: about_icon,
        h_cursor: LoadCursorW(std::ptr::null_mut(), 32512usize as LPCWSTR),
        hbr_background: std::ptr::null_mut(),
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: about_class.as_ptr(),
        h_icon_sm: about_icon,
    };
    RegisterClassExW(&wc_about);
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
            return Err(lang::tpl(
                lang::t().timeout,
                &[("secs", &secs.to_string())],
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
    Err(lang::tpl(
        lang::t().spawn_failed,
        &[("err", &last_err)],
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
        Err(e) => return Err(lang::tpl(lang::t().cmd_spawn_failed, &[("err", &e.to_string())])),
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
        return Err(lang::tpl(
            lang::t().timeout,
            &[("secs", &secs.to_string())],
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
        lang::tpl(lang::t().exit_code, &[("code", &code.to_string())])
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
    Err(lang::tpl(
        lang::t().winget_error,
        &[("code", &code_str(out.code)), ("detail", detail.trim())],
    ))
}

/// 升级单个软件包。`silent` 为 true 时追加 `--silent`。
fn upgrade_package(id: &str, silent: bool) -> Result<(), String> {
    let mut args: Vec<&str> = vec![
        "upgrade",
        "--id",
        id,
        "--exact",
        "--include-unknown",
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--disable-interactivity",
    ];
    if silent {
        args.push("--silent");
    }
    let out = winget_run(&args, None)?;
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
        Err(lang::tpl(
            lang::t().winget_failed,
            &[("code", &code_str(out.code))],
        ))
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
        fallback: bool,
    },
    SilentFallback {
        index: usize,
    },
    UpgradeDone {
        ok: usize,
        fail: usize,
        fallbacks: usize,
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
    progress: HWND,
    btn_all: HWND,
    btn_none: HWND,
    btn_upgrade: HWND,
    btn_back: HWND,
    btn_about: HWND,
    theme_label: HWND,
    theme_btns: [HWND; 5],
    lang_label: HWND,
    lang_btns: [HWND; 3],
    silent_label: HWND,
    silent_btns: [HWND; 2],
    phase: Phase,
    items: Vec<Item>,
    tx: Sender<TaskMsg>,
    rx: Receiver<TaskMsg>,
    timer_on: bool,
    total: usize,
    finished: usize,
    ok: usize,
    fail: usize,
    /// 开始升级时的勾选快照（升级期间锁定勾选状态）
    upgrade_checked: Vec<bool>,
    /// 每一行的结果状态：0=未处理 1=成功 2=失败 3=等待/进行中
    results: Vec<u8>,
    /// 当前正在升级的行
    current_index: Option<usize>,
    current_name: String,
    current_n: usize,
    current_start: Option<std::time::Instant>,
    last_status_tick: Option<std::time::Instant>,
    font: HFONT,
    title_font: HFONT,
    dpi: i32,
    spinner_frame: i64,
    spinner_rect: RECT,
    theme_choice: theme::Choice,
    lang_choice: config::Lang,
    config: config::Config,
    col_w: [i32; 5],
    head_pad: i32,
    header_rect: RECT,
    list_rect: RECT,
    state_images: HIMAGELIST,
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
    unsafe fn new(hwnd: HWND, cfg: config::Config) -> Box<App> {
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
        let class_button = wide("WPButton");
        let class_list = wide("SysListView32");
        UI_FONT = font;

        // 初始界面
        let title = make_child(
            hwnd, instance, &class_static, lang::t().app_name,
            SS_CENTER | SS_NOPREFIX, 0, 0, s(38), s(820), s(40), ID_TITLE, title_font,
        );
        let subtitle = make_child(
            hwnd, instance, &class_static, lang::t().subtitle,
            SS_CENTER | SS_NOPREFIX, 0, 0, s(92), s(820), s(26), ID_SUBTITLE, font,
        );
        let btn_view = make_child(
            hwnd, instance, &class_button, lang::t().view,
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(250), s(170), s(150), s(44), ID_VIEW, font,
        );
        let btn_exit = make_child(
            hwnd, instance, &class_button, lang::t().exit,
            BS_PUSHBUTTON | WS_TABSTOP, 0, s(420), s(170), s(150), s(44), ID_EXIT, font,
        );

        // 清单界面
        let status = make_child(
            hwnd, instance, &class_static, "",
            SS_LEFT | SS_NOPREFIX, 0, s(40), s(10), s(764), s(22), ID_STATUS, font,
        );
        let spinner_rect = RECT {
            left: s(16),
            top: s(13),
            right: s(16) + s(16),
            bottom: s(13) + s(16),
        };
        let class_progress = wide("msctls_progress32");
        let progress = make_child(
            hwnd, instance, &class_progress, "",
            PBS_SMOOTH, 0, s(16), s(36), s(788), s(12), ID_PROGRESS, font,
        );
        // 清单区域：自绘表头 + 无边框 ListView（边框由主窗口绘制，颜色跟随主题）
        let header_rect = RECT {
            left: s(16),
            top: s(52),
            right: s(16) + s(788),
            bottom: s(52) + s(27),
        };
        let list_rect = RECT {
            left: s(16),
            top: s(79),
            right: s(16) + s(788),
            bottom: s(79) + s(385),
        };
        let list_w = list_rect.right - list_rect.left;
        let fixed_w = s(105) + s(105) + s(96);
        let remain = (list_w - fixed_w).max(s(240));
        let name_w = remain * 60 / 100;
        let id_w = remain - name_w;
        let col_w = [name_w, s(105), s(105), id_w, s(96)];

        let list = make_child(
            hwnd,
            instance,
            &class_list,
            "",
            LVS_REPORT | LVS_SHOWSELALWAYS | LVS_NOCOLUMNHEADER,
            0,
            list_rect.left,
            list_rect.top,
            list_w,
            list_rect.bottom - list_rect.top,
            ID_LIST,
            font,
        );
        if !list.is_null() {
            let ex_style =
                (LVS_EX_FULLROWSELECT | LVS_EX_CHECKBOXES | LVS_EX_DOUBLEBUFFER) as LPARAM;
            SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, ex_style);
            for (i, title) in lang::t().columns.iter().enumerate() {
                let t = wide(title);
                let mut col = LVCOLUMNW::default();
                col.mask = LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM;
                col.fmt = LVCFMT_LEFT;
                col.cx = col_w[i];
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
        }
        // 清单界面底部动作按钮：统一 150×32
        let list_btn_w = s(150);
        let list_btn_gap = s(8);
        let list_x = s(16);
        let btn_all = make_child(
            hwnd,
            instance,
            &class_button,
            lang::t().all,
            BS_PUSHBUTTON | WS_TABSTOP,
            0,
            list_x,
            s(472),
            list_btn_w,
            s(32),
            ID_ALL,
            font,
        );
        let btn_none = make_child(
            hwnd,
            instance,
            &class_button,
            lang::t().none,
            BS_PUSHBUTTON | WS_TABSTOP,
            0,
            list_x + list_btn_w + list_btn_gap,
            s(472),
            list_btn_w,
            s(32),
            ID_NONE,
            font,
        );
        let btn_upgrade = make_child(
            hwnd,
            instance,
            &class_button,
            &lang::tpl(lang::t().upgrade_button, &[("n", "0")]),
            BS_PUSHBUTTON | WS_TABSTOP,
            0,
            list_x + (list_btn_w + list_btn_gap) * 2,
            s(472),
            list_btn_w,
            s(32),
            ID_UPGRADE,
            font,
        );
        let btn_back = make_child(
            hwnd,
            instance,
            &class_button,
            lang::t().back,
            BS_PUSHBUTTON | WS_TABSTOP,
            0,
            s(804) - list_btn_w,
            s(472),
            list_btn_w,
            s(32),
            ID_BACK,
            font,
        );

        // 设置区：2×2 表格。设置按钮统一 80×26，标签右对齐并与按钮保持 8px 间距。
        let row1_y = s(400);
        let row2_y = s(436);
        let ctl_h = s(26);
        let left_x = s(36);
        let label_w = s(64);
        let label_gap = s(8);
        let btn_w = s(80);
        let btn_gap = s(6);

        // 第一列第一行：主题
        let theme_label = make_child(
            hwnd,
            instance,
            &class_static,
            lang::t().theme_label,
            SS_RIGHT | SS_NOPREFIX,
            0,
            left_x,
            row1_y,
            label_w,
            ctl_h,
            ID_THEME_LABEL,
            font,
        );
        let mut theme_x = left_x + label_w + label_gap;
        let mut theme_btns: [HWND; 5] = [std::ptr::null_mut(); 5];
        for (i, _choice) in theme::CHOICES.iter().enumerate() {
            theme_btns[i] = make_child(
                hwnd,
                instance,
                &class_button,
                lang::t().theme_labels[i],
                WS_TABSTOP,
                0,
                theme_x,
                row1_y,
                btn_w,
                ctl_h,
                ID_THEME_BASE + i as i32,
                font,
            );
            theme_x += btn_w + btn_gap;
        }

        // 第一列第二行：语言
        let lang_label = make_child(
            hwnd,
            instance,
            &class_static,
            lang::t().lang_label,
            SS_RIGHT | SS_NOPREFIX,
            0,
            left_x,
            row2_y,
            label_w,
            ctl_h,
            ID_LANG_LABEL,
            font,
        );
        let mut lang_x = left_x + label_w + label_gap;
        let mut lang_btns: [HWND; 3] = [std::ptr::null_mut(); 3];
        for (i, text) in lang::t().lang_labels.iter().enumerate() {
            lang_btns[i] = make_child(
                hwnd,
                instance,
                &class_button,
                text,
                WS_TABSTOP,
                0,
                lang_x,
                row2_y,
                btn_w,
                ctl_h,
                ID_LANG_BASE + i as i32,
                font,
            );
            lang_x += btn_w;
            if i + 1 < lang_btns.len() {
                lang_x += btn_gap;
            }
        }

        // 第二列所在区域：右半区，内容居中
        let col2_left = s(544);
        let col2_right = s(804);
        let col2_w = col2_right - col2_left;

        // 第二列第一行：静默升级
        let silent_group_w = label_w + label_gap + btn_w * 2 + btn_gap;
        let silent_x = col2_left + (col2_w - silent_group_w) / 2;
        let silent_label = make_child(
            hwnd,
            instance,
            &class_static,
            lang::t().silent_label,
            SS_RIGHT | SS_NOPREFIX,
            0,
            silent_x,
            row1_y,
            label_w,
            ctl_h,
            ID_SILENT_LABEL,
            font,
        );
        let silent_buttons_x = silent_x + label_w + label_gap;
        let silent_btns: [HWND; 2] = [
            make_child(
                hwnd,
                instance,
                &class_button,
                lang::t().silent_off,
                WS_TABSTOP,
                0,
                silent_buttons_x,
                row1_y,
                btn_w,
                ctl_h,
                ID_SILENT_OFF,
                font,
            ),
            make_child(
                hwnd,
                instance,
                &class_button,
                lang::t().silent_on,
                WS_TABSTOP,
                0,
                silent_buttons_x + btn_w + btn_gap,
                row1_y,
                btn_w,
                ctl_h,
                ID_SILENT_ON,
                font,
            ),
        ];

        // 第二列第二行：关于
        let about_x = col2_left + (col2_w - btn_w) / 2;
        let btn_about = make_child(
            hwnd,
            instance,
            &class_button,
            lang::t().about,
            WS_TABSTOP,
            0,
            about_x,
            row2_y,
            btn_w,
            ctl_h,
            ID_ABOUT,
            font,
        );

        let (tx, rx) = channel();

        let initial_choice = cfg.theme;
        let initial_lang = cfg.lang;
        let initial_silent = cfg.silent;
        let mut app = Box::new(App {
            hwnd,
            title,
            subtitle,
            btn_view,
            btn_exit,
            status,
            list,
            progress,
            btn_all,
            btn_none,
            btn_upgrade,
            btn_back,
            btn_about,
            theme_label,
            theme_btns,
            lang_label,
            lang_btns,
            silent_label,
            silent_btns,
            phase: Phase::Init,
            items: Vec::new(),
            tx,
            rx,
            timer_on: false,
            total: 0,
            finished: 0,
            ok: 0,
            fail: 0,
            upgrade_checked: Vec::new(),
            results: Vec::new(),
            current_index: None,
            current_name: String::new(),
            current_n: 0,
            current_start: None,
            last_status_tick: None,
            font,
            title_font,
            dpi,
            spinner_frame: 0,
            spinner_rect,
            theme_choice: initial_choice,
            lang_choice: initial_lang,
            config: cfg,
            col_w,
            head_pad: s(22),
            header_rect,
            list_rect,
            state_images: std::ptr::null_mut(),
        });
        SendMessageW(progress, PBM_SETRANGE32, 0, 100);
        for (i, btn) in app.theme_btns.iter().enumerate() {
            set_button_selected(*btn, initial_choice.index() == i);
        }
        for (i, btn) in app.lang_btns.iter().enumerate() {
            set_button_selected(*btn, initial_lang.index() == i);
        }
        for (i, btn) in app.silent_btns.iter().enumerate() {
            set_button_selected(*btn, (i == 1) == initial_silent);
        }
        app.apply_theme_visual(initial_choice);
        app.apply_phase();
        app
    }

    /// 应用主题到所有自绘控件（不写配置）。
    unsafe fn apply_theme_visual(&mut self, choice: theme::Choice) {
        self.theme_choice = choice;
        let idx = choice.resolve_index(theme::system_uses_dark());
        theme::set_active_index(idx);
        let th = *theme::active();

        if !ui_brush_bg().is_null() {
            DeleteObject(ui_brush_bg() as HGDIOBJ);
        }
        UI_BRUSH_BG = CreateSolidBrush(th.bg);
        set_dark_titlebar(self.hwnd, th.dark);

        if !self.list.is_null() {
            SendMessageW(self.list, LVM_SETBKCOLOR, 0, th.list_bg as LPARAM);
            SendMessageW(self.list, LVM_SETTEXTCOLOR, 0, th.text as LPARAM);
            SendMessageW(self.list, LVM_SETTEXTBKCOLOR, 0, th.list_bg as LPARAM);
            let images = make_state_images(&th);
            if !images.is_null() {
                SendMessageW(self.list, LVM_SETIMAGELIST, LVSIL_STATE, images as LPARAM);
                if !self.state_images.is_null() {
                    ImageList_Destroy(self.state_images);
                }
                self.state_images = images;
            }
            self.invalidate_list();
        }

        if !self.progress.is_null() {
            set_classic_theme(self.progress);
            SendMessageW(self.progress, PBM_SETBKCOLOR, 0, th.progress_track as LPARAM);
            SendMessageW(self.progress, PBM_SETBARCOLOR, 0, th.accent as LPARAM);
        }

        for (i, btn) in self.theme_btns.iter().enumerate() {
            set_button_selected(*btn, choice.index() == i);
        }

        InvalidateRect(self.hwnd, std::ptr::null(), 1);
        let repaint = [
            self.title,
            self.subtitle,
            self.status,
            self.progress,
            self.theme_label,
            self.theme_btns[0],
            self.theme_btns[1],
            self.theme_btns[2],
            self.theme_btns[3],
            self.theme_btns[4],
            self.lang_label,
            self.lang_btns[0],
            self.lang_btns[1],
            self.lang_btns[2],
            self.silent_label,
            self.silent_btns[0],
            self.silent_btns[1],
            self.btn_about,
            self.btn_view,
            self.btn_exit,
            self.btn_all,
            self.btn_none,
            self.btn_upgrade,
            self.btn_back,
        ];
        for h in repaint {
            if !h.is_null() {
                InvalidateRect(h, std::ptr::null(), 1);
            }
        }
        self.config.theme = choice;
    }

    /// 用户切换主题：应用 + 写配置。
    fn set_theme(&mut self, choice: theme::Choice) {
        unsafe {
            self.apply_theme_visual(choice);
        }
        self.config.save();
    }

    /// 用户切换语言：应用 + 写配置。
    fn set_lang(&mut self, choice: config::Lang) {
        unsafe {
            self.apply_language_visual(choice);
        }
        self.config.save();
    }

    /// 用户切换静默升级：更新选中态 + 写配置。
    fn set_silent(&mut self, on: bool) {
        self.config.silent = on;
        unsafe {
            set_button_selected(self.silent_btns[0], !on);
            set_button_selected(self.silent_btns[1], on);
        }
        self.config.save();
    }

    /// 应用语言到所有界面文案（不写配置）。
    unsafe fn apply_language_visual(&mut self, choice: config::Lang) {
        self.lang_choice = choice;
        let idx = lang::resolve(choice, system_ui_is_chinese());
        lang::set_index(idx);
        let t = lang::t();

        let title = wide(t.app_name);
        SetWindowTextW(self.hwnd, title.as_ptr());
        set_text(self.title, t.app_name);
        set_text(self.subtitle, t.subtitle);
        set_text(self.btn_view, t.view);
        set_text(self.btn_exit, t.exit);
        set_text(self.btn_all, t.all);
        set_text(self.btn_none, t.none);
        set_text(self.btn_back, t.back);
        set_text(self.theme_label, t.theme_label);
        set_text(self.lang_label, t.lang_label);
        set_text(self.btn_about, t.about);
        set_text(self.silent_label, t.silent_label);
        set_text(self.silent_btns[0], t.silent_off);
        set_text(self.silent_btns[1], t.silent_on);
        let silent = self.config.silent;
        set_button_selected(self.silent_btns[0], !silent);
        set_button_selected(self.silent_btns[1], silent);
        for (i, btn) in self.theme_btns.iter().enumerate() {
            set_text(*btn, t.theme_labels[i]);
        }
        for (i, btn) in self.lang_btns.iter().enumerate() {
            set_text(*btn, t.lang_labels[i]);
            set_button_selected(*btn, choice.index() == i);
        }
        self.refresh_upgrade_button();
        InvalidateRect(self.hwnd, std::ptr::null(), 1);
        let repaint = [
            self.title,
            self.subtitle,
            self.theme_label,
            self.lang_label,
            self.btn_view,
            self.btn_exit,
            self.btn_all,
            self.btn_none,
            self.btn_upgrade,
            self.btn_back,
            self.btn_about,
            self.silent_label,
            self.silent_btns[0],
            self.silent_btns[1],
        ];
        for h in repaint {
            if !h.is_null() {
                InvalidateRect(h, std::ptr::null(), 1);
            }
        }
        self.config.lang = choice;
    }

    /// 主窗口客户区绘制：自绘表头 + ListView 边框。
    unsafe fn paint(&self, hdc: HDC) {
        if self.phase == Phase::Init {
            return;
        }
        let th = *theme::active();
        let hr = self.header_rect;

        let header_bg = CreateSolidBrush(th.header_bg);
        FillRect(hdc, &hr, header_bg);
        DeleteObject(header_bg as HGDIOBJ);

        let border = CreateSolidBrush(th.border);
        let header_bottom = RECT {
            left: hr.left,
            top: hr.bottom - 1,
            right: hr.right,
            bottom: hr.bottom,
        };
        FillRect(hdc, &header_bottom, border);

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, th.muted);
        SelectObject(hdc, self.font as HGDIOBJ);
        let mut x = hr.left;
        for (i, title) in lang::t().columns.iter().enumerate() {
            let w = self.col_w[i];
            let left_pad = if i == 0 { self.head_pad } else { 0 };
            let mut tr = RECT {
                left: x + 10 + left_pad,
                top: hr.top,
                right: x + w - 6,
                bottom: hr.bottom - 1,
            };
            let t = wide(title);
            DrawTextW(
                hdc,
                t.as_ptr(),
                -1,
                &mut tr,
                0x0000 | DT_VCENTER | DT_SINGLELINE,
            );
            if i + 1 < lang::t().columns.len() {
                let sep = RECT {
                    left: x + w - 1,
                    top: hr.top + 6,
                    right: x + w,
                    bottom: hr.bottom - 7,
                };
                FillRect(hdc, &sep, border);
            }
            x += w;
        }

        // ListView 外框
        let lr = self.list_rect;
        let lines = [
            RECT { left: lr.left - 1, top: lr.top - 1, right: lr.right + 1, bottom: lr.top },
            RECT { left: lr.left - 1, top: lr.bottom, right: lr.right + 1, bottom: lr.bottom + 1 },
            RECT { left: lr.left - 1, top: lr.top - 1, right: lr.left, bottom: lr.bottom + 1 },
            RECT { left: lr.right, top: lr.top - 1, right: lr.right + 1, bottom: lr.bottom + 1 },
        ];
        for line in lines.iter() {
            FillRect(hdc, line, border);
        }
        DeleteObject(border as HGDIOBJ);

        if matches!(self.phase, Phase::Loading | Phase::Upgrading) {
            draw_spinner(hdc, &self.spinner_rect, self.spinner_frame);
        }
    }

    fn apply_phase(&mut self) {
        let phase = self.phase;
        let init = phase == Phase::Init;
        let list_mode = phase == Phase::List;
        let upgrading = phase == Phase::Upgrading;
        show(self.title, init);
        show(self.subtitle, init);
        show(self.btn_view, init);
        show(self.btn_exit, init);
        show(self.status, !init);
        show(self.progress, upgrading);
        show(self.list, !init);
        show(self.btn_all, !init);
        show(self.btn_none, !init);
        show(self.btn_upgrade, !init);
        show(self.btn_back, !init);
        show(self.theme_label, init);
        for btn in self.theme_btns.iter() {
            show(*btn, init);
            enable(*btn, init);
        }
        show(self.lang_label, init);
        for btn in self.lang_btns.iter() {
            show(*btn, init);
            enable(*btn, init);
        }
        show(self.silent_label, init);
        for btn in self.silent_btns.iter() {
            show(*btn, init);
            enable(*btn, init);
        }
        show(self.btn_about, init);
        enable(self.btn_about, init);
        // 阶段切换时清掉残留的转圈像素（尤其是升级结束回到初始界面）
        unsafe {
            let rc = self.spinner_rect;
            InvalidateRect(self.hwnd, &rc, 1);
        }
        // 升级期间列表保持可用：可以滚动/查看进度，只是勾选会被锁定
        enable(self.list, list_mode || upgrading);
        enable(self.btn_all, list_mode);
        enable(self.btn_none, list_mode);
        enable(self.btn_back, list_mode);
        enable(self.btn_upgrade, false);
        if list_mode {
            self.refresh_upgrade_button();
            unsafe {
                SetFocus(self.list);
            }
        } else if upgrading {
            unsafe {
                SetFocus(self.list);
            }
        } else if init {
            unsafe {
                SetFocus(self.btn_view);
            }
        }
    }

    fn is_checked(&self, index: usize) -> bool {
        let state = unsafe {
            SendMessageW(self.list, LVM_GETITEMSTATE, index, LVIS_STATEIMAGEMASK as LPARAM)
        } as u32;
        (state >> 12) == 2
    }

    fn checked_indices(&self) -> Vec<usize> {
        let count = unsafe { SendMessageW(self.list, LVM_GETITEMCOUNT, 0, 0) } as usize;
        (0..count).filter(|&i| self.is_checked(i)).collect()
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
        let text = lang::tpl(lang::t().upgrade_button, &[("n", &n.to_string())]);
        set_text(self.btn_upgrade, &text);
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

    fn set_progress(&self, pos: usize) {
        unsafe {
            SendMessageW(self.progress, PBM_SETPOS, pos, 0);
        }
    }

    fn invalidate_list(&self) {
        unsafe {
            InvalidateRect(self.list, std::ptr::null(), 0);
        }
    }

    fn upgrade_status_text(&self, secs: u64) -> String {
        let n = self.current_n.to_string();
        let total = self.total.to_string();
        let ok = self.ok.to_string();
        let fail = self.fail.to_string();
        let secs = secs.to_string();
        let name = truncate_chars(&self.current_name, 48);
        lang::tpl(
            lang::t().upgrade_status,
            &[
                ("n", &n),
                ("total", &total),
                ("ok", &ok),
                ("fail", &fail),
                ("secs", &secs),
                ("name", &name),
            ],
        )
    }

    fn refresh_upgrade_status(&self) {
        let secs = self
            .current_start
            .map(|s| s.elapsed().as_secs())
            .unwrap_or(0);
        self.set_status(&self.upgrade_status_text(secs));
    }

    /// 升级期间定时刷新状态（“已用 Ns”），让用户知道当前包仍在处理中
    fn tick_upgrade_status(&mut self) {
        if self.phase != Phase::Upgrading || self.current_start.is_none() {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.last_status_tick {
            if now.duration_since(last).as_millis() < 500 {
                return;
            }
        }
        self.last_status_tick = Some(now);
        self.refresh_upgrade_status();
    }

    /// 加载/升级期间推动 spinner 旋转（复用 80ms 定时器）。
    fn tick_spinner(&mut self) {
        if !matches!(self.phase, Phase::Loading | Phase::Upgrading) {
            return;
        }
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        unsafe {
            let rc = self.spinner_rect;
            InvalidateRect(self.hwnd, &rc, 1);
        }
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
        self.set_status(lang::t().fetching);
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
        self.results = vec![0u8; self.items.len()];
        self.upgrade_checked.clear();
        for i in 0..self.items.len() {
            self.insert_row(i);
        }
        let n = self.items.len();
        self.phase = Phase::List;
        self.set_status(&lang::t().list_ready(n));
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
        // 记录勾选快照，升级期间保持锁定
        self.upgrade_checked = (0..self.items.len()).map(|i| self.is_checked(i)).collect();
        self.results = vec![0u8; self.items.len()];
        for &i in &indices {
            if let Some(item) = self.items.get(i) {
                selected.push((i, item.clone()));
                self.set_result(i, lang::t().waiting);
                if let Some(r) = self.results.get_mut(i) {
                    *r = 3;
                }
            }
        }
        self.phase = Phase::Upgrading;
        self.total = total;
        self.finished = 0;
        self.ok = 0;
        self.fail = 0;
        self.current_index = None;
        self.current_name.clear();
        self.current_n = 0;
        self.current_start = None;
        self.last_status_tick = None;
        self.set_progress(0);
        self.set_status(&lang::tpl(
            lang::t().preparing,
            &[("total", &total.to_string())],
        ));
        self.apply_phase();
        unsafe {
            SendMessageW(self.progress, PBM_SETRANGE32, 0, total as LPARAM);
        }
        self.start_timer();
        let silent = self.config.silent;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let total = selected.len();
            let mut ok = 0usize;
            let mut fallbacks = 0usize;
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
                let result = upgrade_package(&item.id, silent);
                if silent {
                    if let Err(first) = &result {
                        let _ = tx.send(TaskMsg::SilentFallback { index });
                        match upgrade_package(&item.id, false) {
                            Ok(()) => {
                                fallbacks += 1;
                                ok += 1;
                                let _ = tx.send(TaskMsg::ProgressResult {
                                    index,
                                    ok: true,
                                    fallback: true,
                                });
                                continue;
                            }
                            Err(second) => {
                                let detail = lang::tpl(
                                    lang::t().silent_fallback_failed,
                                    &[("first", first), ("second", &second)],
                                );
                                failures.push((item.name.clone(), detail));
                                let _ = tx.send(TaskMsg::ProgressResult {
                                    index,
                                    ok: false,
                                    fallback: true,
                                });
                                continue;
                            }
                        }
                    }
                }
                match result {
                    Ok(()) => {
                        ok += 1;
                        let _ = tx.send(TaskMsg::ProgressResult {
                            index,
                            ok: true,
                            fallback: false,
                        });
                    }
                    Err(e) => {
                        failures.push((item.name.clone(), e));
                        let _ = tx.send(TaskMsg::ProgressResult {
                            index,
                            ok: false,
                            fallback: false,
                        });
                    }
                }
            }
            let _ = tx.send(TaskMsg::UpgradeDone {
                ok,
                fail: failures.len(),
                fallbacks,
                failures,
            });
        });
    }

    fn reset_to_init(&mut self) {
        self.stop_timer();
        self.phase = Phase::Init;
        self.items.clear();
        self.results.clear();
        self.upgrade_checked.clear();
        self.current_index = None;
        self.current_name.clear();
        self.current_n = 0;
        self.current_start = None;
        self.last_status_tick = None;
        unsafe {
            SendMessageW(self.list, LVM_DELETEALLITEMS, 0, 0);
            SendMessageW(self.progress, PBM_SETPOS, 0, 0);
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
                        Some(lang::tpl(lang::t().fetch_failed, &[("e", &e)]))
                    }
                    Ok(items) if items.is_empty() => {
                        self.reset_to_init();
                        Some(lang::t().no_updates.to_string())
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
                self.current_index = Some(index);
                self.current_name = name;
                self.current_n = n;
                self.total = total;
                self.current_start = Some(std::time::Instant::now());
                self.last_status_tick = Some(std::time::Instant::now());
                if let Some(r) = self.results.get_mut(index) {
                    if *r != 1 && *r != 2 {
                        *r = 3;
                    }
                }
                self.set_result(index, lang::t().upgrading);
                self.refresh_upgrade_status();
                self.set_progress(self.finished);
                unsafe {
                    // 当前行不在可视范围时自动滚动过去，方便看到“正在升级”的行
                    SendMessageW(self.list, LVM_ENSUREVISIBLE, index, 0);
                }
                self.invalidate_list();
                None
            }
            TaskMsg::SilentFallback { index } => {
                self.set_status(lang::t().silent_retrying);
                self.set_result(index, lang::t().upgrading);
                self.invalidate_list();
                None
            }
            TaskMsg::ProgressResult {
                index,
                ok,
                fallback: _,
            } => {
                let result_text = if ok {
                    lang::t().ok
                } else {
                    lang::t().failed
                };
                self.set_result(index, result_text);
                if let Some(r) = self.results.get_mut(index) {
                    *r = if ok { 1 } else { 2 };
                }
                self.current_index = None;
                self.current_start = None;
                self.finished += 1;
                if ok {
                    self.ok += 1;
                } else {
                    self.fail += 1;
                }
                self.set_progress(self.finished);
                let done_text = lang::tpl(
                    lang::t().done_status,
                    &[
                        ("finished", &self.finished.to_string()),
                        ("total", &self.total.to_string()),
                        ("ok", &self.ok.to_string()),
                        ("fail", &self.fail.to_string()),
                    ],
                );
                self.set_status(&done_text);
                self.invalidate_list();
                None
            }
            TaskMsg::UpgradeDone {
                ok,
                fail,
                fallbacks,
                failures,
            } => {
                self.stop_timer();
                let mut text = if fail == 0 {
                    lang::t().summary_all_ok_text(ok)
                } else {
                    lang::tpl(
                        lang::t().summary_done,
                        &[("ok", &ok.to_string()), ("fail", &fail.to_string())],
                    )
                };
                if !failures.is_empty() {
                    text.push_str(lang::t().summary_failures);
                    for (i, (name, detail)) in failures.iter().take(5).enumerate() {
                        text.push_str(&format!("\n{}. {}", i + 1, name));
                        let d = detail.replace('\n', " ");
                        let d = d.trim();
                        if !d.is_empty() {
                            text.push_str(&format!("\n    {}", truncate_chars(d, 140)));
                        }
                    }
                    if failures.len() > 5 {
                        text.push_str(&lang::tpl(
                            lang::t().summary_more,
                            &[("n", &(failures.len() - 5).to_string())],
                        ));
                    }
                }
                if fallbacks > 0 {
                    text.push_str("\n\n");
                    text.push_str(&lang::tpl(
                        lang::t().silent_fallback_note,
                        &[("n", &fallbacks.to_string())],
                    ));
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
                    return Some(lang::t().select_hint.to_string());
                }
                if self.config.silent {
                    let warn = lang::tpl(
                        lang::t().silent_warning,
                        &[("n", &indices.len().to_string())],
                    );
                    if message_box_ex(
                        self.hwnd,
                        &warn,
                        MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                    ) != IDYES
                    {
                        return None;
                    }
                }
                self.begin_upgrade(indices);
                None
            }
            ID_SILENT_OFF => {
                self.set_silent(false);
                None
            }
            ID_SILENT_ON => {
                self.set_silent(true);
                None
            }
            ID_ABOUT => {
                unsafe {
                    show_about(self.hwnd, self.font, self.title_font, self.dpi);
                    if !self.btn_about.is_null() {
                        SetFocus(self.btn_about);
                    }
                }
                None
            }
            id if (ID_THEME_BASE..=ID_THEME_BASE + 4).contains(&id) => {
                let index = (id - ID_THEME_BASE) as usize;
                if let Some(choice) = theme::Choice::from_index(index) {
                    self.set_theme(choice);
                }
                None
            }
            id if (ID_LANG_BASE..=ID_LANG_BASE + 2).contains(&id) => {
                let index = (id - ID_LANG_BASE) as usize;
                if let Some(choice) = config::Lang::from_index(index) {
                    self.set_lang(choice);
                }
                None
            }
            _ => None,
        }
    }

    unsafe fn on_notify(&mut self, lparam: LPARAM) -> LRESULT {
        if lparam == 0 {
            return 0;
        }
        let hdr = &*(lparam as *const NMHDR);
        if hdr.id_from != ID_LIST as usize {
            return 0;
        }
        match hdr.code {
            LVN_ITEMCHANGED => {
                if self.phase == Phase::List {
                    self.refresh_upgrade_button();
                } else if self.phase == Phase::Upgrading {
                    // 升级期间锁定勾选：任何勾选变化都还原到开始升级时的快照
                    let nmlv = &*(lparam as *const NMLISTVIEW);
                    let index = nmlv.i_item;
                    if index >= 0 && (index as usize) < self.items.len() {
                        let want = self
                            .upgrade_checked
                            .get(index as usize)
                            .copied()
                            .unwrap_or(false);
                        if self.is_checked(index as usize) != want {
                            self.set_checked(index as usize, want);
                        }
                    }
                }
                0
            }
            NM_DBLCLK => {
                if self.phase == Phase::List {
                    let nmlv = &*(lparam as *const NMLISTVIEW);
                    if nmlv.i_item >= 0 {
                        let index = nmlv.i_item as usize;
                        if index < self.items.len() {
                            let on = !self.is_checked(index);
                            self.set_checked(index, on);
                            self.refresh_upgrade_button();
                        }
                    }
                }
                0
            }
            NM_CUSTOMDRAW => self.on_custom_draw(lparam),
            _ => 0,
        }
    }

    /// 行着色：选中/当前行高亮背景；结果列按成功/失败/等待着色；POSTPAINT 阶段自绘分隔线。
    unsafe fn on_custom_draw(&self, lparam: LPARAM) -> LRESULT {
        let cd = &mut *(lparam as *mut NMLVCUSTOMDRAW);
        let stage = cd.nmcd.draw_stage;
        let th = *theme::active();
        if stage == CDDS_PREPAINT {
            return CDRF_NOTIFYITEMDRAW;
        }
        if stage == CDDS_POSTPAINT {
            // 自绘行/列分隔线（替代系统亮色 gridlines，且只画有数据的行）
            let list = cd.nmcd.hdr.hwnd_from;
            let hdc = cd.nmcd.hdc;
            let count = SendMessageW(list, LVM_GETITEMCOUNT, 0, 0) as usize;
            let top = SendMessageW(list, LVM_GETTOPINDEX, 0, 0) as usize;
            let total: i32 = self.col_w.iter().sum();
            let brush = CreateSolidBrush(th.grid);
            for i in top..count {
                let mut rc: RECT = std::mem::zeroed();
                let ok = SendMessageW(list, LVM_GETITEMRECT, i, &mut rc as *mut _ as LPARAM);
                if ok == 0 {
                    continue;
                }
                let bottom = RECT {
                    left: 0,
                    top: rc.bottom - 1,
                    right: total,
                    bottom: rc.bottom,
                };
                FillRect(hdc, &bottom, brush);
                let mut x = 0i32;
                for w in self.col_w.iter() {
                    x += *w;
                    if x >= total {
                        break;
                    }
                    let v = RECT {
                        left: x - 1,
                        top: rc.top,
                        right: x,
                        bottom: rc.bottom - 1,
                    };
                    FillRect(hdc, &v, brush);
                }
            }
            DeleteObject(brush as HGDIOBJ);
            return CDRF_DODEFAULT;
        }
        if stage & CDDS_ITEMPREPAINT == 0 {
            return CDRF_DODEFAULT;
        }

        let list = cd.nmcd.hdr.hwnd_from;
        let index = cd.nmcd.item_spec as usize;
        let is_current = self.phase == Phase::Upgrading && self.current_index == Some(index);
        let selected_state = SendMessageW(
            list,
            LVM_GETITEMSTATE,
            index,
            LVIS_SELECTED as LPARAM,
        ) as u32;
        let is_selected = selected_state & LVIS_SELECTED != 0;
        let row_bg = if is_current || is_selected {
            th.accent_soft
        } else {
            th.list_bg
        };

        if stage & CDDS_SUBITEM != 0 {
            cd.clr_text_bk = row_bg;
            cd.clr_text = th.text;
            if cd.i_sub_item == 4 {
                match self.results.get(index).copied().unwrap_or(0) {
                    1 => cd.clr_text = th.ok,
                    2 => cd.clr_text = th.err,
                    3 => cd.clr_text = th.muted,
                    _ => {}
                }
            }
            return CDRF_NEWFONT;
        }
        cd.clr_text = th.text;
        cd.clr_text_bk = row_bg;
        CDRF_NOTIFYSUBITEMDRAW | CDRF_NEWFONT
    }

    fn on_close(&mut self) -> CloseDecision {
        if self.phase == Phase::Upgrading {
            self.stop_timer();
            CloseDecision::Confirm(lang::t().close_confirm.to_string())
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
                Phase::Init => {
                    let mut v = vec![app.btn_view, app.btn_exit];
                    v.extend(app.theme_btns.iter().copied());
                    v.extend(app.lang_btns.iter().copied());
                    v.push(app.btn_about);
                    v
                }
                Phase::List => vec![
                    app.list,
                    app.btn_all,
                    app.btn_none,
                    app.btn_upgrade,
                    app.btn_back,
                ],
                Phase::Upgrading => vec![app.list],
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
        WM_ERASEBKGND => {
            let hdc = wparam as HDC;
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let brush = ui_brush_bg();
            if !brush.is_null() {
                FillRect(hdc, &rc, brush);
            }
            1
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            with_app(|app| app.paint(hdc));
            EndPaint(hwnd, &ps);
            0
        }
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
            let mut result: LRESULT = 0;
            with_app(|app| result = app.on_notify(lparam));
            result
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
            with_app(|app| app.tick_upgrade_status());
            with_app(|app| app.tick_spinner());
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
            let th = *theme::active();
            SetBkMode(hdc, TRANSPARENT);
            SetBkColor(hdc, th.bg);
            let id = GetDlgCtrlID(lparam as HWND);
            let color = if id == ID_SUBTITLE || id == ID_THEME_LABEL {
                th.muted
            } else {
                th.text
            };
            SetTextColor(hdc, color);
            ui_brush_bg() as LRESULT
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
            dw_icc: ICC_LISTVIEW_CLASSES | ICC_PROGRESS_CLASS,
        };
        let _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("WingetPickerWnd");
        let class_ptr = class_name.as_ptr();

        // 读取便携配置（主题/语言/静默），并注册自绘按钮窗口类
        let cfg = config::Config::load();
        let system_zh = system_ui_is_chinese();
        lang::set_index(lang::resolve(cfg.lang, system_zh));
        theme::set_active_index(cfg.theme.resolve_index(theme::system_uses_dark()));
        register_control_classes(instance);

        // 图标资源（build.rs 编译 assets/app.rc；没有资源时回退系统默认图标）
        let mut app_icon = LoadIconW(instance, 1usize as LPCWSTR);
        if app_icon.is_null() {
            app_icon = LoadIconW(std::ptr::null_mut(), 32512usize as LPCWSTR);
        }

        let wc = WNDCLASSEXW {
            cb_size: std::mem::size_of::<WNDCLASSEXW>() as UINT,
            style: 0,
            lpfn_wnd_proc: Some(wndproc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: instance,
            h_icon: app_icon,
            h_cursor: LoadCursorW(std::ptr::null_mut(), 32512usize as LPCWSTR),
            hbr_background: std::ptr::null_mut(),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_ptr,
            h_icon_sm: app_icon,
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
        let title = wide(lang::t().app_name);
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
            message_box(
                std::ptr::null_mut(),
                lang::t().window_create_failed,
                MB_OK | MB_ICONWARNING,
            );
            return;
        }

        let app = App::new(hwnd, cfg);
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
