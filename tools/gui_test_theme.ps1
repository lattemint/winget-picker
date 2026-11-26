param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$FakeDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\fake-winget",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens-themes"
)

# 主题 / 便携配置回归：
#   1. 5 套主题初始界面截图（light/dark/nord/nord-light/auto）
#   2. 深色下列表页 + 全选后自定义勾选框截图
#   3. ini 记忆：写入主题后重启应恢复
# 需要先用 rustc 编译 fake-winget，见 README。
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ThemeTest {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
}
"@
Add-Type -AssemblyName System.Drawing
[ThemeTest]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Snap([IntPtr]$h, [string]$name) {
    $r = New-Object ThemeTest+RECT
    [ThemeTest]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [ThemeTest]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    $path = Join-Path $OutDir $name
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output "saved $path"
}

function Get-WindowText2([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 512
    [ThemeTest]::GetWindowTextW($h, $sb, 512) | Out-Null
    return $sb.ToString()
}

function Stop-Apps {
    Get-Process | Where-Object { $_.ProcessName -like '*winget-picker*' -or $_.ProcessName -like 'wp-test*' } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

function Wait-Window($p) {
    $deadline = [DateTime]::Now.AddSeconds(10)
    while ([DateTime]::Now -lt $deadline) {
        if ($p.MainWindowHandle -ne 0) { return $p.MainWindowHandle }
        Start-Sleep -Milliseconds 150
    }
    return [IntPtr]::Zero
}

$results = @()
Stop-Apps

# 1) 5 套主题（含跟随系统）
foreach ($theme in 'light', 'dark', 'nord', 'nord-light', 'auto') {
    if ($theme -eq 'auto') {
        Remove-Item Env:WINGET_PICKER_THEME -ErrorAction SilentlyContinue
    } else {
        $env:WINGET_PICKER_THEME = $theme
    }
    $env:WINGET_PICKER_NO_SAVE = '1'
    $p = Start-Process -FilePath $Exe -PassThru
    $hwnd = Wait-Window $p
    if ($hwnd -eq [IntPtr]::Zero) { throw "主题 $theme 启动后未找到窗口" }
    Start-Sleep -Milliseconds 500
    Snap $hwnd "theme-$theme.png"
    $results += "theme_$theme=ok"
    if (!$p.HasExited) { $p.Kill() }
    Start-Sleep -Milliseconds 250
}

# 2) 深色列表 + 勾选框
$env:WINGET_PICKER_THEME = 'dark'
$env:WINGET_PICKER_NO_SAVE = '1'
$OrigPath = $env:PATH
$env:PATH = "$FakeDir;$env:PATH"
Stop-Apps
$p = Start-Process -FilePath $Exe -WorkingDirectory $FakeDir -PassThru
$hwnd = Wait-Window $p
if ($hwnd -eq [IntPtr]::Zero) { throw "列表测试启动后未找到窗口" }
[ThemeTest]::SendMessageW($hwnd, 0x111, [IntPtr]1001, [ThemeTest]::GetDlgItem($hwnd, 1001)) | Out-Null
$status = [ThemeTest]::GetDlgItem($hwnd, 1009)
$deadline = [DateTime]::Now.AddSeconds(30)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 250
    if ((Get-WindowText2 $status) -match '个可更新软件') { break }
}
Start-Sleep -Milliseconds 400
Snap $hwnd "list-dark-unchecked.png"
[ThemeTest]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [ThemeTest]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 500
Snap $hwnd "list-dark-checked.png"
$results += "list_dark=ok"
[ThemeTest]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 300
if (!$p.HasExited) { $p.Kill() }

# 3) ini 记忆重启
$ini = Join-Path $env:TEMP "winget-picker-theme-test.ini"
Set-Content -Path $ini -Value "theme=nord`nlang=auto`nsilent=0`n" -Encoding ASCII
$env:WINGET_PICKER_INI = $ini
Remove-Item Env:WINGET_PICKER_THEME -ErrorAction SilentlyContinue
Stop-Apps
$p = Start-Process -FilePath $Exe -PassThru
$hwnd = Wait-Window $p
Start-Sleep -Milliseconds 500
Snap $hwnd "persist-nord.png"
$results += "persist_ini=ok"
if (!$p.HasExited) { $p.Kill() }
Remove-Item Env:WINGET_PICKER_INI -ErrorAction SilentlyContinue

# 4) 深色 → 浅色切换后，底部标签/按钮必须重绘成新主题颜色
$env:WINGET_PICKER_THEME = 'dark'
$env:WINGET_PICKER_NO_SAVE = '1'
Stop-Apps
$p = Start-Process -FilePath $Exe -PassThru
$hwnd = Wait-Window $p
if ($hwnd -eq [IntPtr]::Zero) { throw "主题重绘测试启动后未找到窗口" }
Start-Sleep -Milliseconds 500
# 1021 = 浅色
[ThemeTest]::SendMessageW($hwnd, 0x111, [IntPtr]1021, [ThemeTest]::GetDlgItem($hwnd, 1021)) | Out-Null
Start-Sleep -Milliseconds 800
Snap $hwnd "theme-dark-to-light.png"
$wr = New-Object ThemeTest+RECT
$cr = New-Object ThemeTest+RECT
[ThemeTest]::GetWindowRect($hwnd, [ref]$wr) | Out-Null
[ThemeTest]::GetClientRect($hwnd, [ref]$cr) | Out-Null
$pt = New-Object ThemeTest+POINT
[ThemeTest]::ClientToScreen($hwnd, [ref]$pt) | Out-Null
$ox = $pt.X - $wr.Left
$oy = $pt.Y - $wr.Top
# (x,y) 取各控件内部空白处：主题标签 / 语言标签 / 静默标签 / 主题按钮 / 语言按钮 / 静默按钮 / 关于按钮
$samples = @(
    @(66, 412), @(66, 448), @(633, 412),
    @(158, 422), @(180, 458), @(688, 422), @(690, 458)
)
$img = New-Object System.Drawing.Bitmap (Join-Path $OutDir "theme-dark-to-light.png")
$bad = @()
foreach ($s in $samples) {
    $c = $img.GetPixel($ox + $s[0], $oy + $s[1])
    if ($c.R -lt 200 -or $c.G -lt 200 -or $c.B -lt 200) {
        $bad += "($($s[0]),$($s[1]))=$($c.R),$($c.G),$($c.B)"
    }
}
$img.Dispose()
if ($bad.Count -gt 0) { throw "深色→浅色后底部控件未重绘：$($bad -join '; ')" }
# 再切回深色，确认同样立即生效
[ThemeTest]::SendMessageW($hwnd, 0x111, [IntPtr]1022, [ThemeTest]::GetDlgItem($hwnd, 1022)) | Out-Null
Start-Sleep -Milliseconds 800
Snap $hwnd "theme-light-to-dark.png"
$img = New-Object System.Drawing.Bitmap (Join-Path $OutDir "theme-light-to-dark.png")
$bright = @()
foreach ($s in @(@(66, 412), @(66, 448), @(633, 412))) {
    $c = $img.GetPixel($ox + $s[0], $oy + $s[1])
    if ($c.R -gt 150) { $bright += "($($s[0]),$($s[1]))=$($c.R),$($c.G),$($c.B)" }
}
$img.Dispose()
if ($bright.Count -gt 0) { throw "浅色→深色后底部控件未重绘：$($bright -join '; ')" }
$results += "theme_switch_repaint=ok"
if (!$p.HasExited) { $p.Kill() }
Stop-Apps

Write-Output ($results -join ' ')
Write-Output "ALL_OK"

if ($OrigPath) { $env:PATH = $OrigPath }
