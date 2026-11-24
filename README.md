# winget-picker（winget 选择性升级工具）

一个体积很小的 Windows 桌面小工具：列出 `winget upgrade` 检测到的可更新软件，
让你**只勾选想升级的那些**，后台逐个升级，避免 `winget upgrade --all` 把不想动的软件也一起升了。

- 语言：Rust（**零第三方依赖**，只有标准库 + 直接调用系统 DLL）
- 界面：原生 Win32（无运行时、无 Electron/Tauri 体积负担）
- 体积：**约 278 KB**（`opt-level="z"` + LTO + strip + panic=abort）
- 平台：Windows 10/11（需系统自带 `winget` / 应用安装程序 App Installer）

## 界面与流程

初始界面只有两个按钮：

- **查看升级**：在后台执行一次
  `winget upgrade --include-unknown --accept-source-agreements --disable-interactivity`，
  解析返回的表格，按**名称**排序后显示为列表（名称 / 当前版本 / 可用版本 / ID / 结果）。
- **退出**：直接关闭，不执行任何 winget 命令。

进入列表后：

- 勾选想升级的软件（点击复选框；也可选中行后按**空格**；**双击行**可切换勾选）
- **全选 / 全不选**：一键勾选所有或清空勾选
- **升级选中 (N)**：后台逐个执行
  `winget upgrade --id <ID> --exact --include-unknown --accept-source-agreements --accept-package-agreements --disable-interactivity`
- 升级过程中：
  - 顶部状态栏显示「正在升级 x/N（成功 a · 失败 b · 已用 Ns）：<软件名>」，
    耗时秒数会持续刷新，方便判断当前包是否仍在处理中
  - 状态栏下方显示**进度条**，直观反映已完成比例
  - **列表不再锁死**：可以正常上下滚动、选中行查看；只有**勾选会被锁定**
    （此期间点击复选框/按空格会自动还原，保证本次升级范围不会被误改）
  - **正在升级的行以浅蓝色背景高亮**，并自动滚动到可视区域；
    “结果”列中 **成功=绿色 / 失败=红色 / 等待中=灰色**
- **返回**（或 Esc）：回到初始界面（升级期间该按钮不可用）
- 一轮升级结束后弹出汇总（成功/失败数量，失败项及原因），然后**自动回到初始界面**

小提示：窗口内按 Tab 切换焦点、Esc 返回，初始界面按 Enter 激活“查看升级”；升级期间可用方向键/滚动条浏览列表。

## 运行

1. 双击 `winget-picker.exe` 即可。
2. 建议放在普通目录（桌面、`C:\Tools`、Program Files 等均可）。

> ⚠️ **本机测试环境提示**：不要在本 DSH 工作区目录（`C:\Users\MECHREVO\Documents\dsh-workspace\...`）
> 里直接运行本程序。该目录带有沙箱 ACL，位于其中的 exe 启动 `winget`（AppX 执行别名）时会被
> Windows 拒绝激活（表现为“winget 返回错误 0x8A150001”）。把 exe 复制到桌面等普通目录即可正常工作。
> 已验证：同一个 exe 在 `C:\Users\MECHREVO\Documents\wp_test\` 下正常、在
> `C:\Users\MECHREVO\Documents\dsh-workspace\wp_test\` 下失败。
>
> 为方便使用，已生成一份可直接运行的副本：`C:\Users\MECHREVO\Desktop\winget-picker.exe`。

## 构建

需要 Rust 工具链（MSVC target）：

```powershell
cd winget-picker
cargo build --release
# 产物：target\release\winget-picker.exe
```

`Cargo.toml` 中已配置：

```toml
[profile.release]
opt-level = "z"      # 体积优先
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

`build.rs` 通过链接器把 `app.manifest` 嵌入 exe（启用 Common Controls v6 视觉样式 +
DPI 感知 + Windows 10/11 兼容声明）。

运行解析单元测试：

```powershell
cargo test --release
```

## 实现要点

- **纯 Win32 GUI**：`RegisterClassExW` / `CreateWindowExW` / ListView / MessageBox，
  无资源文件、无第三方 GUI 库，因此二进制只有 ~278 KB。
- **`winget upgrade` 表格解析**（`parse_upgrade_table`）：
  - 以“全为 `-`/空格的分隔行”定位表头，按**显示列宽**（中文按 2 列）切分单元格，
    与系统语言无关（中/英文 winget 输出都适用）；
  - 处理了“列名与数据间只有 1 个空格”“名称填满整列”“名称/版本里含多空格或中文”
    “末列较短导致整行比表格窄（源列 winget/msstore 宽度不同）”“结尾统计行需跳过”等情况；
  - 用真实 `winget` 输出做回归测试（47 条样本），并另有合成用例。
- **后台执行**：查看列表、逐个升级都在工作线程中进行，主线程只处理 UI 消息；
  winget 输出用并发读管道 + 有界等待，避免管道阻塞/挂死。
- **健壮性**：
  - 列表获取有 120 秒超时；
  - 若直接启动 winget 失败且没有任何输出（部分环境下 AppX 激活异常），自动改用
    `cmd.exe` 中转 + 临时文件重定向重试一次；
  - 程序启动时分配隐藏控制台供子进程继承（GUI 程序调用 winget 的兼容性保险）。
- **零依赖**：`Cargo.lock` 中没有任何第三方 crate。

## 已验证内容

- 5 个解析单元测试全部通过（真实中文 winget 输出 + 合成中英文表格）。
- **真实 winget**：程序从桌面运行时成功执行 `winget upgrade` 并列出本机 47 个可更新软件
  （截图 `screens-real/02_list_real.png`）；返回初始界面、关闭窗口均正常。
- **升级流程**（用“假 winget”验证，避免改动真实软件）：全选 47 项后完整执行
  1 次列表 + 47 次 `winget upgrade --id ... --exact ...`，进度、成功/失败标记、
  结束汇总（含失败项）与“一轮后回到初始界面”均符合预期（截图 `screens/04_upgrading.png`、
  `screens/05_summary.png`）。
- **升级过程 UI**（`tools/gui_test_progress.ps1` 自动验证，截图 `screens-progress/30_upgrading_scrollable.png`、
  `screens-progress/31_autoscroll_progress.png`）：
  - 列表仍可滚动（升级中发送滚动消息后 top index 0→3）；
  - 勾选被锁定（升级中按空格尝试取消勾选，状态自动还原）；
  - 当前行自动滚入可视区（升级到第 30 项时 top index=12）并以浅蓝背景高亮；
  - “结果”列颜色正确：成功=绿色、失败=红色、等待中=灰色（像素采样验证）；
  - 进度条与“已用 Ns”计时正常刷新。
- **退出按钮**：点击后立即退出，未启动任何 winget 进程。
- 真实升级未在测试中执行（避免未经许可改动本机软件），但升级命令的构造与调用已由假 winget 验证。

## 文件说明

```
winget-picker/
├── Cargo.toml / Cargo.lock
├── build.rs               # 嵌入 app.manifest
├── app.manifest           # 视觉样式/DPI/系统兼容清单
├── src/main.rs            # 全部实现（Win32 GUI + winget 调用 + 表格解析 + 测试）
├── testdata/winget_upgrade_zh.txt   # 真实 winget 输出样本（解析回归测试用）
├── fake-winget/           # 测试用假 winget（源码 + 输出样本；测试前编译出 winget.exe）
├── tools/
│   ├── gui_test.ps1          # 用假 winget 跑完整升级流程的自动化测试
│   ├── gui_test_progress.ps1 # 升级过程 UI 回归（可滚动/勾选锁定/自动跟随）
│   └── gui_verify.ps1        # 真实 winget 的界面/列表自动化验证
├── screens/               # 假 winget 全流程截图
├── screens-real/          # 真实 winget 列表截图
├── screens-progress/      # 升级过程 UI（滚动/高亮/进度）截图
└── dist/                  # 本地发布副本目录（构建产物，不提交；可发 GitHub Releases）
```

> 测试相关说明：`target/`、`dist/`、`fake-winget/winget.exe` 等构建产物不纳入版本库。
> 运行 GUI 自动化测试前，先编译假 winget：
> `rustc -O -o fake-winget\winget.exe fake-winget\fake_winget.rs`
> 注意：GUI 测试脚本会调用 `Start-Process`，请把 exe 放在普通目录运行（见上文“运行”一节的环境提示）。

## 已知限制

- 解析针对 `winget upgrade` 的表格输出；若未来 winget 更改表格格式需同步调整解析器。
- 升级需要管理员权限的安装包时，winget 会弹出 UAC 提示，属正常现象。
- 升级过程中关闭窗口会确认一次；已启动的 winget 子进程可能继续在后台完成。
