//! 中英文案表（编译期两份 `Strings`，零依赖）。
//!
//! 带参数的文案用 `{name}` 占位符 + `tpl()` 替换；单复数在 `Strings` 的方法里处理。

use crate::config::Lang;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Strings {
    zh: bool,
    pub app_name: &'static str,
    pub subtitle: &'static str,
    pub view: &'static str,
    pub exit: &'static str,
    pub all: &'static str,
    pub none: &'static str,
    pub back: &'static str,
    pub columns: [&'static str; 5],
    pub theme_label: &'static str,
    pub theme_labels: [&'static str; 5],
    pub lang_label: &'static str,
    pub lang_labels: [&'static str; 3],
    pub upgrade_button: &'static str,
    pub fetching: &'static str,
    pub select_hint: &'static str,
    pub preparing: &'static str,
    pub upgrade_status: &'static str,
    pub done_status: &'static str,
    pub waiting: &'static str,
    pub upgrading: &'static str,
    pub ok: &'static str,
    pub failed: &'static str,
    pub fetch_failed: &'static str,
    pub no_updates: &'static str,
    pub summary_all_ok: &'static str,
    pub summary_done: &'static str,
    pub summary_failures: &'static str,
    pub summary_more: &'static str,
    pub close_confirm: &'static str,
    pub timeout: &'static str,
    pub spawn_failed: &'static str,
    pub cmd_spawn_failed: &'static str,
    pub winget_error: &'static str,
    pub winget_failed: &'static str,
    pub exit_code: &'static str,
    pub window_create_failed: &'static str,
}

pub const ZH: Strings = Strings {
    zh: true,
    app_name: "winget 选择性更新",
    subtitle: "先查看可更新的软件，勾选后再只升级你选择的部分",
    view: "查看升级",
    exit: "退出",
    all: "全选",
    none: "全不选",
    back: "返回",
    columns: ["名称", "当前版本", "可用版本", "ID", "结果"],
    theme_label: "主题",
    theme_labels: ["跟随系统", "浅色", "深色", "Nord 深色", "Nord 浅色"],
    lang_label: "语言",
    lang_labels: ["跟随系统", "中文", "English"],
    upgrade_button: "升级选中 ({n})",
    fetching: "正在获取可更新的软件列表，请稍候…",
    select_hint: "请先勾选需要升级的软件。",
    preparing: "准备升级 {total} 个软件…",
    upgrade_status: "正在升级 {n}/{total}（成功 {ok} · 失败 {fail} · 已用 {secs}s）：{name}",
    done_status: "已完成 {finished}/{total}（成功 {ok}，失败 {fail}）",
    waiting: "等待中",
    upgrading: "升级中…",
    ok: "成功",
    failed: "失败",
    fetch_failed: "获取可更新软件列表失败。\n\n{e}",
    no_updates: "没有检测到可更新的软件，系统当前已是最新状态。",
    summary_all_ok: "本轮升级完成：{ok} 个软件全部升级成功。",
    summary_done: "本轮升级结束：成功 {ok} 个，失败 {fail} 个。",
    summary_failures: "\n\n以下软件未能升级：",
    summary_more: "\n… 另有 {n} 个失败。",
    close_confirm: "升级仍在后台进行中。\n\n确定要退出界面吗？（已启动的 winget 进程可能继续运行）",
    timeout: "winget 响应超时（{secs} 秒）。\n可能是网络或软件源问题，请稍后重试。",
    spawn_failed: "无法启动 winget（{err}）。\n请确认系统已安装“应用安装程序”(App Installer)。",
    cmd_spawn_failed: "无法启动 cmd.exe（{err}）",
    winget_error: "winget 返回错误（{code}）。\n{detail}",
    winget_failed: "winget 失败（{code}）",
    exit_code: "退出码 {code}",
    window_create_failed: "创建窗口失败。",
};

pub const EN: Strings = Strings {
    zh: false,
    app_name: "winget selective upgrade",
    subtitle: "Review available updates, then upgrade only the packages you choose",
    view: "Check upgrades",
    exit: "Exit",
    all: "Select all",
    none: "Select none",
    back: "Back",
    columns: ["Name", "Current", "Available", "ID", "Result"],
    theme_label: "Theme",
    theme_labels: ["Auto", "Light", "Dark", "Nord dark", "Nord light"],
    lang_label: "Language",
    lang_labels: ["Auto", "中文", "English"],
    upgrade_button: "Upgrade selected ({n})",
    fetching: "Fetching available updates…",
    select_hint: "Select at least one package to upgrade.",
    preparing: "Preparing to upgrade {total} packages…",
    upgrade_status: "Upgrading {n}/{total} (OK {ok} · Failed {fail} · {secs}s): {name}",
    done_status: "Done {finished}/{total} (OK {ok}, failed {fail})",
    waiting: "Queued",
    upgrading: "Upgrading…",
    ok: "OK",
    failed: "Failed",
    fetch_failed: "Failed to fetch the update list.\n\n{e}",
    no_updates: "No updates found — everything is up to date.",
    summary_all_ok: "Upgrade finished: {ok} package(s) upgraded successfully.",
    summary_done: "Upgrade finished: {ok} succeeded, {fail} failed.",
    summary_failures: "\n\nThese packages could not be upgraded:",
    summary_more: "\n… {n} more failed.",
    close_confirm: "An upgrade is still running in the background.\n\nExit the window anyway? (Started winget processes may continue.)",
    timeout: "winget timed out after {secs}s.\nCheck your network or sources and try again.",
    spawn_failed: "Could not start winget ({err}).\nMake sure App Installer is installed.",
    cmd_spawn_failed: "Could not start cmd.exe ({err})",
    winget_error: "winget returned an error ({code}).\n{detail}",
    winget_failed: "winget failed ({code})",
    exit_code: "exit code {code}",
    window_create_failed: "Failed to create the window.",
};

impl Strings {
    pub fn list_ready(&self, n: usize) -> String {
        if self.zh {
            format!("共 {n} 个可更新软件。勾选后点击“升级选中”。")
        } else if n == 1 {
            "1 update available. Tick it, then click “Upgrade selected”.".to_string()
        } else {
            format!("{n} updates available. Tick packages, then click “Upgrade selected”.")
        }
    }

    pub fn summary_all_ok_text(&self, ok: usize) -> String {
        if self.zh {
            format!("本轮升级完成：{ok} 个软件全部升级成功。")
        } else if ok == 1 {
            "Upgrade finished: the selected package upgraded successfully.".to_string()
        } else {
            format!("Upgrade finished: all {ok} packages upgraded successfully.")
        }
    }
}

const ALL: [&Strings; 2] = [&ZH, &EN];
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub fn t() -> &'static Strings {
    ALL[ACTIVE.load(Ordering::Relaxed) % ALL.len()]
}

pub fn set_index(index: usize) {
    ACTIVE.store(index % ALL.len(), Ordering::Relaxed);
}

/// 把语言选项解析成实际下标（0=中文，1=英文）。
pub fn resolve(choice: Lang, system_zh: bool) -> usize {
    match choice {
        Lang::Auto => {
            if system_zh {
                0
            } else {
                1
            }
        }
        Lang::Zh => 0,
        Lang::En => 1,
    }
}

pub fn is_zh() -> bool {
    t().zh
}

/// 简单模板替换：`{key}` → value。
pub fn tpl(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in args {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}
