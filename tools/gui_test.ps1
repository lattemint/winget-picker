param(
    [string]$Exe = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\target\release\winget-picker.exe",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens",
    [string]$FakeDir = "",
    [string]$Scenario = "flow"
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class W {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
"@ -ErrorAction Stop
[W]::SetProcessDPIAware() | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OrigPath = $env:PATH
$env:PATH = "$FakeDir;$env:PATH"
$workDir = if ($FakeDir) { $FakeDir } else { Split-Path $Exe }

function Get-WindowText([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 1024
    [W]::GetWindowTextW($h, $sb, 1024) | Out-Null
    return $sb.ToString()
}

function Save-WindowShot([IntPtr]$h, [string]$name) {
    [W]::ShowWindow($h, 9) | Out-Null   # SW_RESTORE
    [W]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Milliseconds 350
    $r = New-Object W+RECT
    [W]::GetWindowRect($h, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left
    $hh = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [W]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $path = Join-Path $OutDir $name
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Output "saved $path"
}

function Save-ScreenShot([string]$name) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
    $path = Join-Path $OutDir $name
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Output "saved $path"
}

function Wait-Window([string]$cls, [string]$title, [int]$timeoutMs) {
    $deadline = [DateTime]::Now.AddMilliseconds($timeoutMs)
    while ([DateTime]::Now -lt $deadline) {
        if ($title) {
            $h = [W]::FindWindowW($cls, $title)
        } else {
            $h = [W]::FindWindowW($cls, [NullString]::Value)
        }
        if ($h -ne [IntPtr]::Zero) { return $h }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

$p = Start-Process -FilePath $Exe -WorkingDirectory $workDir -PassThru
$hwnd = Wait-Window "WingetPickerWnd" $null 8000
if ($hwnd -eq [IntPtr]::Zero) { throw "主窗口未出现" }
Write-Output "hwnd=$hwnd"
Start-Sleep -Milliseconds 500
Save-WindowShot $hwnd "01_initial.png"

if ($Scenario -eq "initial") {
    [W]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 500
    if (!$p.HasExited) { $p.Kill() }
    Write-Output "done"
    exit 0
}

# 点击“查看升级”
$btnView = [W]::GetDlgItem($hwnd, 1001)
[W]::SendMessageW($hwnd, 0x111, [IntPtr]1001, $btnView) | Out-Null
Write-Output "clicked view"

$status = [W]::GetDlgItem($hwnd, 1009)
$listLoaded = $false
$msg = [IntPtr]::Zero
$deadline = [DateTime]::Now.AddSeconds(25)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 200
    $msg = [W]::FindWindowW("#32770", "winget 选择性更新")
    if ($msg -ne [IntPtr]::Zero) { break }
    $st = Get-WindowText $status
    if ($st -match "个可更新软件") { $listLoaded = $true; break }
}
Write-Output "list_loaded=$listLoaded msgbox=$msg status=$(Get-WindowText $status)"
Save-WindowShot $hwnd "02_list.png"

if ($msg -ne [IntPtr]::Zero) {
    Save-WindowShot $msg "03_msgbox.png"
    [W]::SendMessageW($msg, 0x111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 800
    Save-WindowShot $hwnd "04_after_msgbox.png"
    [W]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 500
    if (!$p.HasExited) { $p.Kill() }
    Write-Output "done-msgbox"
    exit 0
}

$btnUp = [W]::GetDlgItem($hwnd, 1005)

if ($Scenario -eq "viewonly") {
    $btnBack = [W]::GetDlgItem($hwnd, 1006)
    [W]::SendMessageW($hwnd, 0x111, [IntPtr]1006, $btnBack) | Out-Null
    Start-Sleep -Milliseconds 600
    Save-WindowShot $hwnd "03_back_to_init.png"
    [W]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 500
    if (!$p.HasExited) { $p.Kill() }
    Write-Output "done-viewonly"
    exit 0
}

if ($Scenario -eq "partial") {
    # 用键盘只勾选第一项：ListView 获得焦点后 Space 切换勾选
    $listCtl = [W]::GetDlgItem($hwnd, 1010)
    [W]::SendMessageW($listCtl, 0x100, [IntPtr]32, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 400
    Write-Output "after_space=$(Get-WindowText $btnUp)"
    if ((Get-WindowText $btnUp) -notmatch '\(1\)') {
        [W]::SendMessageW($listCtl, 0x100, [IntPtr]40, [IntPtr]::Zero) | Out-Null
        Start-Sleep -Milliseconds 300
        [W]::SendMessageW($listCtl, 0x100, [IntPtr]32, [IntPtr]::Zero) | Out-Null
        Start-Sleep -Milliseconds 400
        Write-Output "after_down_space=$(Get-WindowText $btnUp)"
    }
    Save-WindowShot $hwnd "03_partial_checked.png"
    [W]::SendMessageW($hwnd, 0x111, [IntPtr]1005, $btnUp) | Out-Null
    $done = [IntPtr]::Zero
    $deadline = [DateTime]::Now.AddSeconds(30)
    while ([DateTime]::Now -lt $deadline) {
        Start-Sleep -Milliseconds 200
        $done = [W]::FindWindowW("#32770", "winget 选择性更新")
        if ($done -ne [IntPtr]::Zero) { break }
    }
    if ($done -ne [IntPtr]::Zero) {
        Save-WindowShot $done "04_partial_summary.png"
        [W]::SendMessageW($done, 0x111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
        Start-Sleep -Milliseconds 700
        Save-WindowShot $hwnd "05_back_to_init.png"
    } else {
        Write-Output "NO_SUMMARY status=$(Get-WindowText $status)"
    }
    [W]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 500
    if (!$p.HasExited) { $p.Kill() }
    Write-Output "done-partial"
    exit 0
}

# 全选
[W]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [W]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 500
Write-Output "upgrade_button=$(Get-WindowText $btnUp)"
Save-WindowShot $hwnd "03_all_checked.png"

# 升级选中
[W]::SendMessageW($hwnd, 0x111, [IntPtr]1005, $btnUp) | Out-Null
Write-Output "clicked upgrade"
Start-Sleep -Milliseconds 1300
Save-WindowShot $hwnd "04_upgrading.png"

$done = [IntPtr]::Zero
$deadline = [DateTime]::Now.AddSeconds(60)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 300
    $done = [W]::FindWindowW("#32770", "winget 选择性更新")
    if ($done -ne [IntPtr]::Zero) { break }
}
if ($done -eq [IntPtr]::Zero) {
    Write-Output "SUMMARY_NOT_FOUND status=$(Get-WindowText $status)"
    Save-WindowShot $hwnd "05_no_summary.png"
} else {
    Save-WindowShot $done "05_summary.png"
    [W]::SendMessageW($done, 0x111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 800
    Save-WindowShot $hwnd "06_back_to_init.png"
    Write-Output "summary_dismissed"
}

[W]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 500
if (!$p.HasExited) { $p.Kill() }
Write-Output "done"

if ($OrigPath) { $env:PATH = $OrigPath }
