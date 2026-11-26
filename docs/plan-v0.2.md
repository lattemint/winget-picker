# winget-picker v0.2 规划（定稿）· P0 清单

> 状态：**方案定稿，准备动工**。基于 `main.rs`（2232 行，284 KB，零依赖，commit `fec6140`）。
> 本轮范围：5 套主题 / 英文 i18n / 关于 / 加载动画 / 静默升级（含失败回退）/ logo 图标。

## 已确认决策

| 项目 | 决定 |
|---|---|
| GitHub 仓库 | `https://github.com/lattemint/winget-picker/`（关于页链接） |
| 主题集合 | 5 套，与 Beluga CSV 一致：跟随系统 / 浅色 / 深色 / Nord 深色 / Nord 浅色 |
| 选择器形式 | 主界面**底部两行分段按钮**（不弹下拉菜单） |
| 设置保存 | **便携 ini**（exe 同目录优先，不可写时回退 `%LOCALAPPDATA%\winget-picker\`） |
| 版本号 | **0.2.0** |
| 体积 | 不做刻意压缩（预计 325–360 KB） |
| Logo | `wingetpicker.svg`：exe 图标 + 任务栏/窗口图标 + 关于页 + README；**所有尺寸保留文字**，小尺寸变糊属刻意设计 |
| 静默升级 | 设置项默认**关**；开启后升级前提示“可能仍弹窗、UAC 无法跳过”；**静默失败自动回退普通安装** |

## 0. 可行性总结

| 需求 | 可行性 | 难度 | 预计代码量 | 风险 |
|---|---|---|---|---|
| 主题（5 套）+ 分段选择器 | ✅ | 中（Win32 自绘） | ~750–900 行 | 中：ListView/表头/勾选框观感 |
| 关于（GitHub 链接/logo） | ✅ | 低–中 | ~230 行 | 低 |
| 英文 i18n | ✅ | 低–中 | ~380 行 | 低（英文变长需微调布局） |
| 加载转圈动画 | ✅ | 低 | ~90 行 | 低 |
| 静默升级（提示+回退） | ✅ | 低 | ~90 行 + 测试 | 低 |
| Logo/图标资源 | ✅ | 低–中 | ~120 行（资源/绘制/构建） | 低 |

零第三方依赖不变；现有 55 条中文字面量 → 中英 i18n 约 90 条（含新增）。

---

## 0.5 P0 范围（本轮必须完成）

> P0 = 0.2.0 必须做完的；P1/P2 本轮不做，记入后续。

| 编号 | P0 任务 | 依赖 | 验收标准 |
|---|---|---|---|
| **P0-0** ✅ | 主题技术预研 spike：深色窗口/ListView/自绘表头/勾选框/进度条/暗色标题栏 | — | 已完成（2026-11-26），方案定稿，见下方验证结果 |
| **P0-1** ✅ | 基础设施：`Theme`（5 套 token）+ `Strings`（中英）+ 便携 `Config(ini)` + 环境变量覆盖 | P0-0 | 已完成（2026-11-26）：theme.rs / config.rs；单测 6/6 通过；主题持久化验证通过 |
| **P0-2** ✅ | 主题功能：自绘 `WPButton`（含选中态）、窗口背景/静态文字、表头、ListView 行/选中/网格、勾选框、进度条、暗色标题栏；底部两行设置区（5 主题） | P0-1 | 主体完成（2026-11-26）：5 套主题截图、切换/记忆、假 winget 全流程 & 升级中 UI 回归全部通过；图标资源最后接入 |
| **P0-3** ✅ | 英文 i18n：全部界面文案 + 语言分段切换 + 持久化 + 布局校验 | P0-1 | 已完成（2026-11-26）：lang.rs 中英文案表；语言行；英文列表/升级中截图；回归全部通过 |
| **P0-4** ✅ | 关于窗：logo、版本（0.2.0）、GitHub 链接、MIT、双语、主题跟随 | P0-2/P0-3 | 已完成（2026-11-26）：自绘模态窗 + 图标 logo + 可点链接 + 中英/深浅截图 + Esc/Enter/关闭按钮验证 |
| **P0-5** ✅ | 静默升级：开关、升级前提示、失败回退普通安装、汇总提示、i18n | P0-2/P0-3 | 已完成（2026-11-26）：开关/提示/自动回退/汇总说明全部验证通过 |
| **P0-6** ✅ | Spinner：加载列表/升级中状态前转圈 | P0-1 | 已完成（2026-11-26）：主窗口直接绘制的 8 点转圈，加载/升级两帧动画验证通过 |
| **P0-7** ✅ | Logo/图标资源：多尺寸 ico、关于页 png、build.rs rc（可选 rc.exe）、版本资源 | P0-2 | 已完成（随 P0-2）：7 尺寸 ico + 版本资源，资源管理器/关于页显示正确 |
| **P0-8** ✅ | 回归与交付：单测、fake winget 全流程、progress UI、真实列表、主题/语言/关于/静默截图、README、版本 0.2.0、commit | 全部 | 已完成（2026-11-26）：全绿；README 重写、最终截图、0.2.0 发布包与提交完成 |

### P0-0 验证结果（2026-11-26，已完成）

截图：`docs/assets/theme-spike-dark.png`

已验证通过：

- **DWM 暗色标题栏**：`DwmSetWindowAttribute` 属性 20/19 均返回 S_OK，标题栏变暗；
- **深色 ListView**：背景/正文/当前行高亮/结果列颜色（成功绿、失败红、等待灰）全部可控；
- **自绘表头**：隐藏原生 Header（`LVS_NOCOLUMNHEADER` 后原生 header 高度为 0）+ 自绘子窗口，列宽对齐；
- **自绘勾选框**：`ImageList_Create(ILC_COLOR32)` + `LVM_SETIMAGELIST(LVSIL_STATE)` 的三态 16×16 图（空白/未勾/勾选）生效；
- **进度条**：`SetWindowTheme(progress, "", "")` + `PBM_SETBARCOLOR/PBM_SETBKCOLOR` 生效；
- **网格线**：原生 `LVS_EX_GRIDLINES` 是系统亮色、且空行也画线 → 关闭它，改为在 `NM_CUSTOMDRAW` 的 `CDDS_POSTPAINT` 用主题色自绘行/列分隔线（已验证，只画有数据行）；
- **截图注意**：顶层窗口需要 `WS_CLIPCHILDREN`（主程序已是该样式），否则 `PrintWindow` 可能漏掉子控件。

结论：绘制方案定稿，进入 P0-1/P0-2；spike 代码在 `spike/`，实现完成后删除。

### P0-1 / P0-2 实施记录（2026-11-26）

已完成：

- 新增 `src/theme.rs`：4 套实际色板（浅色/深色/Nord 深/Nord 浅）+ `Choice`（含“跟随系统”）+ 注册表读取系统深浅色；
- 新增 `src/config.rs`：便携 ini（exe 同目录 → 不可写回退 `%LOCALAPPDATA%`）+ 环境变量覆盖（`WINGET_PICKER_THEME/LANG/SILENT/INI/NO_SAVE`）；
- `main.rs` 接入主题：自绘 `WPButton`（hover/pressed/disabled/selected/键盘）、自绘表头 + 隐藏原生表头、自定义勾选框 ImageList、ListView 颜色 + 自绘分隔线（POSTPAINT）、经典进度条上色、DWM 暗色标题栏、静态文字/背景随主题；
- 列宽改为按列表实际宽度动态分配（名称/ID 分剩余空间），不再有列表内部右侧空白；表头首列文字增加勾选框缩进；
- 主题切换即时生效并写入 ini；启动读取 ini / 环境变量；“跟随系统”解析系统深浅色；
- 图标资源：`assets/app.ico`（7 尺寸 16..256）、`assets/logo-256.png`、`assets/app.rc`（图标 + 0.2.0 版本信息），`build.rs` 自动调用 rc.exe 编译（找不到 rc.exe 时跳过，不影响构建）；EXE 已带图标/版本信息；
- 验证：5 套主题截图（`screens-themes/`）、ini 记忆重启、鼠标/键盘切换主题、`cargo test --release` 6/6、假 winget 全流程、升级中 UI 回归（ALL_OK）。

待收尾：无（图标资源已接入）。下一步 P0-3 英文 i18n。

### P0-3 实施记录（2026-11-26）

- 新增 `src/lang.rs`：中英两份 `Strings`（界面/主题/语言/状态/错误/汇总约 90 条），`tpl()` 占位符替换，单复数在方法里处理；
- 设置区第二行「语言 [跟随系统][中文][English]」，切换即时生效并写入 ini（`lang=zh/en/auto`），重启保持；
- 窗口标题、静态文字、全部按钮、列标题、结果列、状态栏、汇总、确认框、winget 错误前缀均已本地化；
- 英文布局修正：结果列加宽（`Upgrading…` 不再截断）、列表按钮加宽（Select all / Select none / Upgrade selected (N) / Back）；
- 系统语言检测（`GetUserDefaultUILanguage`）+ `WINGET_PICKER_LANG` 环境变量覆盖；
- 验证：`tools/gui_test_i18n.ps1` **ALL_OK**（中文→英文切换、ini 重启保持、英文列表/升级中状态、切回中文）；主题/升级中/假 winget 回归全部通过；真实 winget 列表 25 项正常。

下一步：P0-4 关于窗。

### P0-4 实施记录（2026-11-26）

- 新增 `WPAbout` 自绘模态窗：图标资源 logo（`LoadImageW` + `DrawIconEx`）、名称/版本（0.2.0）、
  简介、技术副标、GitHub 链接（下划线 + 手型光标，`ShellExecuteW` 打开）、数据/许可说明、关闭按钮；
- 中英双语 + 主题跟随（含 DWM 暗色标题栏）；Esc / Enter / 关闭按钮均可关闭；模态期间主窗禁用、关闭后恢复；
- 主界面第二行右侧新增「关于 / About」按钮；Tab 循环纳入主题/语言/关于按钮；
- 验证：`tools/gui_test_about.ps1`（Esc 关闭、Enter 关闭、owner 禁用/恢复、dark/light × zh/en 截图）**ABOUT_OK**；
  主题/语言/假 winget 回归重跑通过。
- 用户实测断网错误截图已归档到 `docs/assets/offline-error.png`，列入下方 P1「断网/失败提示优化」。

### P0-5 实施记录（2026-11-26）

- 设置区第二行新增 `静默升级 [关][开]`（与语言/关于同一行，整行居中），默认关，写入便携 ini（`silent=0/1`）；
- 开启后点击「升级选中」先弹确认框：可能仍弹安装窗、UAC 无法跳过、失败会自动普通重试（默认按钮为“否”）；
- 升级命令：静默时追加 `--silent`；静默失败自动去掉 `--silent` 重试一次：
  - 回退成功 → 记成功，状态栏显示“静默失败，改用普通方式重试…”，汇总追加“其中 N 个软件静默升级失败，已自动改用普通安装。”；
  - 回退也失败 → 记失败，失败详情同时包含静默与普通两次错误；
- 假 winget 增加模拟：`Git.Git` 在静默模式失败、普通模式成功；`OpenAL` 两种模式都失败；
- 验证：`tools/gui_test_silent.ps1` **SILENT_OK**
  - 静默开：确认框出现且文案含“静默升级/UAC”；`--silent` 调用 47 次；普通回退 2 次（Git/OpenAL）；Git 共 2 次调用；汇总含回退说明；
  - 静默关：无确认框、0 次 `--silent`；
  - 设置持久化：点“开”后 ini 写入 `silent=1`；
- 全流程/升级中 UI/主题/语言回归与单测（6/6）全部通过。

下一步：P0-6 Spinner。

### P0-6 实施记录（2026-11-26）

- 状态文字左侧新增 16×16 加载转圈（8 个点按 frame 旋转、accent→背景渐变），**由主窗口直接绘制**（与表头一致，不额外创建子窗口，避免混合 DPI/截图问题）；
- 复用现有 80ms `WM_TIMER`：加载列表（`Phase::Loading`）与升级中（`Phase::Upgrading`）时每 tick 推进 frame 并只失效转圈区域，空闲/列表就绪时自动不绘制；
- 状态文字右移 24px 给转圈让位；主题/语言切换后颜色自动跟随；
- 假 winget 新增 `FAKE_WINGET_LIST_DELAY_MS` 环境变量，用于稳定复现“加载列表”阶段；
- 验证：`tools/gui_test_spinner.ps1` **SPINNER_OK**
  - 加载阶段：spinner 非背景像素 66、两帧差异 66；
  - 列表就绪：spinner 区域非背景像素 0（已隐藏）；
  - 升级中：非背景 66、两帧差异 66（动画）。
- 顺带修复测试脚本会把整个进程 PATH 删掉的问题（改为保存/恢复 PATH）。
- 全流程/升级中/静默/语言/主题回归与单测（6/6）全部通过。

下一步：P0-7 收尾（README、最终截图、0.2.0 打包、提交）。

### P0-8 实施记录（2026-11-26）

- README 重写为 0.2 版：补主题/语言/关于/静默升级/Spinner、配置 ini、测试脚本、文件结构、已知限制、更新记录；
  致谢写入 `Code by Deepseek v4.1 Flash (1017)`；旧 README 备份为 `README.v0.1-backup.md` 并加入 `.gitignore`；
- README 按 `lieflat-less-ai-tone` 流程检测：脚本层 + 语义层 12 条候选判读，语义四项均低于门槛；
  按建议拆了 4 处句内分号（L37/L105/L147/L149），其余 X 级清单/顿号清单/结构指标按规则保留；
- 最终 exe：346,624 B（约 339 KiB），版本 0.2.0.0；桌面 `winget-picker.exe` 与 `dist/winget-picker.exe` 已更新；
- 发布包：`dist/winget-picker-0.2.0-win64.zip`（exe + README + LICENSE）；
- 真实 winget 最终验证：25 个可更新项正常列出（`screens-real/`）；真实升级未执行；
- 最终回归：假 winget 全流程 / 升级中 UI / 主题 / 语言 / 静默升级 / Spinner / 单测 6/6 全部通过；
- 最终截图重新生成：`screens/`、`screens-progress/`、`screens-themes/`、`screens-real/`；
- 提交：README、plan、最终截图与 .gitignore；打本地 tag `v0.2.0`。

### 0.2.0 界面修正（2026-11-26，发布前）

- 设置区从两行横排改为 **2×2 布局**：第一列主题/语言，第二列静默升级/关于居中；设置按钮统一 **80×26**，标签右对齐并与按钮固定 8px 间距；
- 修复主题切换后底部标签、按钮不重绘（补全 `apply_theme_visual` 的控件失效列表）；
- 修复升级结束回到初始界面后残留转圈（`apply_phase` 阶段切换时失效转圈区域）；
- 测试补充：
  - `gui_test_theme.ps1` 新增深色→浅色→深色重绘断言与截图；
  - `gui_test_spinner.ps1` 新增升级结束返回主界面后转圈清除断言；
  - GUI 测试统一固定主题/语言/静默环境变量，避免用户本地 ini 干扰；
- 重新构建 exe：346,624 B，重新生成截图与发布包。

### P1（本轮不做，记录）

- **断网/失败提示优化**（用户实测，证据 `docs/assets/offline-error.png`）：
  - 现状：断网时直接把 winget/msstore 原始错误丢进汇总框（`WinHttpSendRequest: 12007`、`0x80072ee7`、`unknown error`，且部分文字乱码），系统 MessageBox 标题栏颜色也不随主题；
  - 计划：识别常见网络错误码（12007 / 0x80072ee7 等）→ 显示本地化友好提示（“网络不可用，请检查连接后重试”），原始错误折叠为详情；修复 winget 输出编码（UTF-8/CP_ACP 混用导致乱码）；汇总/提示框是否改为自绘主题窗口以统一暗色标题栏。
- 暗色滚动条（自绘滚动条）
- Microsoft Store 包认证窗口：`--authentication-mode silentPreferred`
- 高对比度/系统强调色适配
- 列宽拖拽/排序、窗口尺寸记忆

### P2（以后视需求）

- 升级完成后系统通知/托盘
- 多语言（除中英）
- 升级历史记录

---

## 1. 主题（暗色 / Nord）

### 1.1 实现方式

Win32 无系统级暗色，但控件少、已有自绘基础，全部可控：

| 区域 | 实现方式 | 难度 |
|---|---|---|
| 窗口背景 / 静态文字 | `WM_ERASEBKGND` + `WM_CTLCOLORSTATIC` | 低 |
| 按钮（查看/退出/全选/…/设置区） | 自定义 `WPButton` 窗口类：normal/hover/pressed/disabled/focus + 选中态；`EnableWindow` 语义不变 | 中（~180 行） |
| ListView 本体 | `LVM_SETBKCOLOR/SETTEXTCOLOR/SETTEXTBKCOLOR` + 扩展现有 `NM_CUSTOMDRAW`（行高亮/结果色/网格） | 中 |
| ListView 表头 | **自绘表头**：原生表头隐藏（`LVS_NOCOLUMNHEADER`），在列表上方放自绘表头子窗口（颜色/列宽共用一份常量） | 中（~110 行） |
| 勾选框 | 自定义 state image list（`LVSIL_STATE`，16×16 按主题自绘勾选/未勾选） | 中（~70 行） |
| 进度条 | `SetWindowTheme(progress, L"", L"")` 经典样式 + `PBM_SETBARCOLOR/SETBKCOLOR` | 低 |
| 标题栏 | 动态 `dwmapi.dll` + `DWMWA_USE_IMMERSIVE_DARK_MODE`（Win10 2004+/11，失败静默） | 低 |
| 滚动条 | 保持系统浅色（P1 可自绘） | —（限制） |

> 表头方案说明：原生 Header 的 owner-draw 消息会先发给 ListView 而不是我们的窗口，子类化链路较绕；
> 而本程序列宽固定、无排序/拖拽需求，**自绘表头 + `LVS_NOCOLUMNHEADER`** 更简单、颜色完全可控，选它。

### 1.2 技术预研（P0-0，1–1.5 小时）

一次性验证程序（不改主程序）：
1. 深色窗口 + 深色 ListView（文字/行背景/选中行/网格）；
2. 自绘表头与列表列对齐；
3. 自定义勾选框 state image；
4. 经典进度条上色 + DWM 暗色标题栏；
5. 截图确认（含浅色滚动条观感）。

通过后删 spike，进入 P0-1。

### 1.3 色板（与 Beluga CSV 对齐）

选项：`跟随系统` / `浅色` / `深色` / `Nord 深色` / `Nord 浅色`（token 从 `Beluga_csv_editor/src/styles.css` 移植）

| token | 浅色 | 深色 | Nord 深色 |
|---|---|---|---|
| window 背景 | `#f5f6f8` | `#14171c` | `#2e3440` |
| 列表背景 | `#ffffff` | `#1b1f26` | `#2b313d` |
| 表头/面板 | `#f2f4f7` | `#22272f` | `#3b4252` |
| 边框/网格 | `#e2e6eb` / `#eef0f3` | `#2c333d` / `#262c34` | `#3b4252` / `#3b4252` |
| 正文 | `#1f2328` | `#e6e9ee` | `#d8dee9` |
| 次要文字 | `#6b7280` | `#9aa4b2` | `#99a1b0` |
| 强调色 | `#2563eb` | `#3b82f6` | `#88c0d0` |
| 当前行高亮 | `#eef3ff` | `#1b2740` | `#3b4252` |
| 成功 / 失败 | `#15803d` / `#b42318` | `#7ec699` / `#f07178` | `#a3be8c` / `#bf616a` |
| 按钮 bg / hover / pressed | `#ffffff` / `#f2f4f7` / `#e2e6eb` | `#22272f` / `#282e38` / `#1b1f26` | `#3b4252` / `#434c5e` / `#2e3440` |

### 1.4 设置区布局（底部两行）

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│                   winget 选择性更新                           │
│         先查看可更新的软件，勾选后再只升级你选择的部分           │
│                                                              │
│               [   查看升级   ]     [   退出   ]               │
│                                                              │
│      主题  [跟随系统] [浅色] [深色] [Nord 深] [Nord 浅]        │
│   语言 [中文] [English]   静默升级 [关] [开]        [关于]     │
└──────────────────────────────────────────────────────────────┘
```

- 主题/语言当前项显示选中态；设置只在初始界面显示，列表/升级界面继承。
- 客户区维持 820×520。

### 1.5 便携配置（ini）

- 路径：`WINGET_PICKER_INI`（测试） → exe 同目录 `winget-picker.ini` → 不可写回退 `%LOCALAPPDATA%\winget-picker\winget-picker.ini`
- 内容：
  ```ini
  theme=dark
  lang=zh
  silent=0
  ```
- 解析/写入失败静默；默认 `theme=auto`、`lang=auto`、`silent=0`
- 测试钩子：`WINGET_PICKER_THEME/LANG/SILENT` 覆盖，`WINGET_PICKER_NO_SAVE=1` 禁止写盘

---

## 2. 关于（About）

- 自绘模态小窗（约 460×300，主题跟随，暗色标题栏），内容：logo、名称+版本、一句话简介、技术副标、GitHub 链接（`ShellExecuteW`）、MIT 许可、关闭按钮；Esc/Enter 可关。
- 文案（待你最终修订）：

  中文：
  ```
  winget-picker  v0.2.0
  升级你勾选的软件——避免 winget upgrade --all 动到不想升级的程序。
  Rust + 原生 Win32

  GitHub 项目主页与问题反馈：github.com/lattemint/winget-picker
  数据来源：本机 winget。程序不联网、不收集任何数据。
  开源协议：MIT License © 2026 LatteMint
  ```
  English：
  ```
  winget-picker  v0.2.0
  Upgrade the packages you select — so `winget upgrade --all` never touches apps you want to keep.
  Rust + raw Win32

  Project & issues: github.com/lattemint/winget-picker
  Data comes from your local winget. The app has no network access and collects no data.
  License: MIT © 2026 LatteMint
  ```

---

## 3. 英文 i18n

- 编译期 `static ZH/EN: Strings`，零依赖；约 90 条词条。
- 带参文案用模板 + 占位符替换函数（`{n}/{total}/{name}/{secs}/{count}`…）。
- 语言：`跟随系统` / `中文` / `English`；系统判断 `GetUserDefaultUILanguage`（`zh*`→中文）。
- 主题名沿用 BelugaCSV：Follow system / Light / Dark / Nord dark / Nord light。
- 英文变长处理：设置区两行；按钮宽度按当前语言重算；状态栏名称截断按像素宽度计算。
- winget 自身输出不翻译，仅加本地化前缀。

---

## 4. 加载转圈动画（Spinner）

- 自绘 `WPSpinner`（14×14，DPI 缩放），8 点按 frame 旋转，主题 accent 渐变；复用 80ms `WM_TIMER`；空闲隐藏。
- 位置：状态文字左侧（状态文本右移 ~20px）。
- 显示：`正在获取可更新的软件列表…` 与 `正在升级…` 两种 busy 状态。
- 可选：加载期间进度条 `PBS_MARQUEE`。

---

## 5. 静默升级（开关 + 提示 + 失败回退）

### 5.1 行为

1. 设置区：`静默升级 [关] [开]`，默认 **关**，持久化到 ini（`silent=0/1`）。
2. 开启时，用户点击「升级选中 (N)」→ 先弹提示框（默认按钮为「取消」）：
   - 中文：
     > **静默升级提示**
     > 已开启静默升级。部分安装器可能仍会弹出安装窗口；需要管理员权限时，UAC 提权窗口无法跳过。
     > 如果静默升级失败，程序会自动改用普通方式重试。
     > 继续升级选中的 N 个软件吗？
   - English：
     > **Silent upgrade**
     > Silent upgrade is on. Some installers may still show their own window, and UAC prompts cannot be skipped when administrator rights are required.
     > If a silent upgrade fails, the app will retry it normally.
     > Continue upgrading the N selected package(s)?
3. 升级命令：
   ```
   winget upgrade --id <ID> --exact --include-unknown --accept-source-agreements \
       --accept-package-agreements --disable-interactivity --silent
   ```
4. **失败回退**：静默命令失败时，自动去掉 `--silent` 用普通方式重试一次：
   - 重试成功 → 该软件记为成功，并统计“静默回退成功数”；
   - 重试失败 → 记为失败（失败详情包含静默+普通两次错误）；
   - 升级状态栏重试期间显示：`静默失败，改用普通方式重试…`。
5. 汇总窗口追加一句（有回退时）：`其中 N 个软件静默升级失败，已自动改用普通安装。`

### 5.2 已知限制

- 安装器本身不支持静默 → 可能仍弹 UI 或直接失败（由回退兜底）。
- 管理员权限安装包 → UAC 无法绕过。
- Microsoft Store 包认证窗口 → P1 再加 `--authentication-mode silentPreferred`。

### 5.3 测试

- 假 winget 记录参数：开关开时含 `--silent`；关时不含。
- 假 winget 增加“对某 ID 在带 --silent 时失败、不带时成功”的行为 → 验证自动回退与汇总文案。
- 真实升级不执行。

---

## 6. Logo / 图标

素材：`wingetpicker.svg`（300×300，白底圆角方块 + 蓝色 `>` + `_` + “WINGET-PICKER”，字体 OCR A Extended，本机已装）。

| 用途 | 产物 | 说明 |
|---|---|---|
| exe 文件图标 | `assets/app.ico`（16/24/32/48/64/128/256） | build.rs 调用 SDK `rc.exe` 编译进 exe；rc.exe 不存在时跳过，构建不失败 |
| 窗口/任务栏图标 | 同一 ico（`LoadIconW` / `WM_SETICON`） | 运行时读取资源 |
| 关于页 logo | `assets/logo-256.png`（透明） | `include_bytes!` + GDI+ 绘制 |
| README/GitHub | `wingetpicker.svg` 直接使用 | GitHub 原生渲染 |
| 版本资源 | rc 内 `VS_VERSION_INFO` | 资源管理器显示 0.2.0 / 产品名 |

- **所有尺寸保留完整 logo 与文字**；小尺寸下文字变糊是刻意设计，不做简化版。
- 生成脚本 `tools/make_icon_assets.ps1`（Edge headless 渲染 SVG → PNG → 打包 ICO），本机运行一次，产物提交仓库；用户/构建者不需要 Edge。
- 图标/关于页使用已生成位图，不依赖 OCR A Extended；以后重生成资源需该字体（或 SVG 转曲）。

---

## 7. 实施顺序

| 阶段 | 内容 | 预计 |
|---|---|---|
| P0-0 | 主题 spike | 1–1.5h |
| P0-1 | 基础设施（Theme/Strings/Config） | 2–3h |
| P0-2 | 主题功能 + 设置区 + logo/ico | 6–8h |
| P0-3 | 英文 i18n | 4–5h |
| P0-4 | 关于窗 | 2–3h |
| P0-5 | 静默升级（提示+回退） | 1.5–2h |
| P0-6 | Spinner | 1–1.5h |
| P0-7 | 回归 + 文档 + 截图 + 0.2.0 + commit | 3h |

合计约 **22–28 小时**（约 2–3 个专注工作日，可分多轮交付）。

每阶段结束跑：
- `cargo test --release`
- `tools/gui_test.ps1`（假 winget 全流程）
- `tools/gui_test_progress.ps1`（升级中 UI 回归）
- 桌面副本 + 真实 winget 列表一次

新增自动化：
- `tools/gui_test_theme.ps1`：环境变量组合启动，截图 + 采样背景色验证主题；关于窗截图；英文状态栏校验。
- 静默升级测试：扩展 fake winget（记录参数 + 静默失败模拟），断言 `--silent` 与自动回退。
- 新截图目录 `screens-themes/`。

---

## 8. 体积与性能

- 当前 290,816 B（284 KB）；预计 325–360 KB。
- 零第三方 crate；新增仅绘制/配置读取，性能影响可忽略。

## 9. 风险

1. 深色 ListView/表头/勾选框观感 → P0-0 spike 前置验证。
2. 滚动条浅色（P1）。
3. 英文文案变长 → 两行设置区 + 逐语言截图。
4. ini 不可写 → 回退 LOCALAPPDATA；失败静默。
5. rc.exe 缺失 → build.rs 容错跳过图标资源。
6. 静默失败回退已兜底，但仍可能两次都失败（照常记为失败）。
7. 高对比度/系统强调色未适配（P1）。
8. 现有行高亮/成功失败硬编码色改主题 token，浅色观感需回归。

## 10. 涉及文件

- `src/main.rs` + 新增模块：`theme.rs`、`lang.rs`、`config.rs`、`controls.rs`、`about.rs`、`icon.rs`
- `assets/app.ico`、`assets/logo-256.png`、`assets/app.rc`
- `build.rs`（rc 资源）、`Cargo.toml`（0.2.0）
- `README.md`、`docs/plan-v0.2.md`、`docs/assets/logo-*.png`
- `tools/make_icon_assets.ps1`、`tools/gui_test_theme.ps1`、`screens-themes/`
- `wingetpicker.svg` 保留为图标源
