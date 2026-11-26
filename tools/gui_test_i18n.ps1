param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$FakeDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\fake-winget",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens-themes"
)

# 语言 i18n 回归：
#   1. 中文启动 → 点 English → 标题/按钮变英文并写入 ini
#   2. 重启（同一 ini）→ 保持英文
#   3. 英文列表页 / 升级中状态文案（假 winget）
#   4. 切回中文
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class I18N {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
}
"@
Add-Type -AssemblyName System.Drawing
[I18N]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Snap([IntPtr]$h, [string]$name) {
    $r = New-Object I18N+RECT
    [I18N]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [I18N]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    $bmp.Save((Join-Path $OutDir $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output "saved $name"
}
function Get-Text([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 512
    [I18N]::GetWindowTextW($h, $sb, 512) | Out-Null
    return $sb.ToString()
}
function Wait-Window($p) {
    $deadline = [DateTime]::Now.AddSeconds(10)
    while ([DateTime]::Now -lt $deadline) {
        if ($p.MainWindowHandle -ne 0) { return $p.MainWindowHandle }
        Start-Sleep -Milliseconds 150
    }
    return [IntPtr]::Zero
}
function Stop-Apps {
    Get-Process | Where-Object { $_.ProcessName -like '*winget-picker*' -or $_.ProcessName -like 'wp-test*' } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

$ini = Join-Path $env:TEMP 'winget-picker-i18n-test.ini'
Remove-Item $ini -ErrorAction SilentlyContinue
$env:WINGET_PICKER_INI = $ini
$env:WINGET_PICKER_NO_SAVE = '0'
Remove-Item Env:WINGET_PICKER_LANG -ErrorAction SilentlyContinue
Remove-Item Env:WINGET_PICKER_THEME -ErrorAction SilentlyContinue
Stop-Apps

# 1) 默认中文 → 切 English
$p = Start-Process -FilePath $Exe -PassThru
$hwnd = Wait-Window $p
Start-Sleep -Milliseconds 500
$title0 = Get-Text $hwnd
if ($title0 -notmatch '选择性更新') { throw "默认应为中文，实际标题：$title0" }
[I18N]::SendMessageW($hwnd, 0x111, [IntPtr]1032, [I18N]::GetDlgItem($hwnd, 1032)) | Out-Null
Start-Sleep -Milliseconds 500
$title1 = Get-Text $hwnd
if ($title1 -notmatch 'selective upgrade') { throw "切换英文失败，标题：$title1" }
Snap $hwnd 'lang-en-initial.png'
$iniText = Get-Content $ini -ErrorAction SilentlyContinue | Out-String
if ($iniText -notmatch 'lang=en') { throw "ini 未写入 lang=en：$iniText" }
$p.Kill()
Start-Sleep -Milliseconds 300

# 2) 重启保持英文
Stop-Apps
$p = Start-Process -FilePath $Exe -PassThru
$hwnd = Wait-Window $p
Start-Sleep -Milliseconds 500
$title2 = Get-Text $hwnd
if ($title2 -notmatch 'selective upgrade') { throw "重启未保持英文，标题：$title2" }
Write-Output "persist_ok title=[$title2]"

# 3) 英文列表 + 升级中状态（假 winget）
$OrigPath = $env:PATH
$env:PATH = "$FakeDir;$env:PATH"
Stop-Apps
$p = Start-Process -FilePath $Exe -WorkingDirectory $FakeDir -PassThru
$hwnd = Wait-Window $p
[I18N]::SendMessageW($hwnd, 0x111, [IntPtr]1001, [I18N]::GetDlgItem($hwnd, 1001)) | Out-Null
$status = [I18N]::GetDlgItem($hwnd, 1009)
$deadline = [DateTime]::Now.AddSeconds(30)
$listReady = $false
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 250
    $s = Get-Text $status
    if ($s -match 'updates available') { $listReady = $true; break }
}
if (!$listReady) { throw "英文列表状态未出现：$(Get-Text $status)" }
Snap $hwnd 'lang-en-list.png'
[I18N]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [I18N]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 300
[I18N]::SendMessageW($hwnd, 0x111, [IntPtr]1005, [I18N]::GetDlgItem($hwnd, 1005)) | Out-Null
Start-Sleep -Milliseconds 1500
$up = Get-Text $status
if ($up -notmatch 'Upgrading') { throw "英文升级状态未出现：$up" }
Snap $hwnd 'lang-en-upgrading.png'
Write-Output "upgrade_status=[$up]"
$p.Kill()

# 4) 切回中文
Stop-Apps
$env:PATH = $OrigPath
$p = Start-Process -FilePath $Exe -PassThru
$hwnd = Wait-Window $p
Start-Sleep -Milliseconds 500
[I18N]::SendMessageW($hwnd, 0x111, [IntPtr]1031, [I18N]::GetDlgItem($hwnd, 1031)) | Out-Null
Start-Sleep -Milliseconds 500
$title3 = Get-Text $hwnd
if ($title3 -notmatch '选择性更新') { throw "切回中文失败，标题：$title3" }
Snap $hwnd 'lang-zh-initial.png'
$p.Kill()
Remove-Item Env:WINGET_PICKER_INI -ErrorAction SilentlyContinue

Write-Output "i18n_ok"
Write-Output "ALL_OK"
