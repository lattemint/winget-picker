param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$FakeDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\fake-winget",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens-themes"
)

# 静默升级回归：
#   1) 静默开：升级前出现确认框；调用带 --silent；Git.Git 静默失败后自动普通重试成功；
#      汇总里出现“静默升级失败，已自动改用普通安装”；OpenAL 两次都失败计入失败。
#   2) 静默关：无确认框；调用不带 --silent。
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class S {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc proc, IntPtr l);
    public static List<string> ChildTexts(IntPtr parent) {
        var list = new List<string>();
        EnumChildWindows(parent, (h, l) => {
            var sb = new StringBuilder(1024);
            GetWindowTextW(h, sb, 1024);
            var t = sb.ToString();
            if (t.Length > 0) list.Add(t);
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
"@
Add-Type -AssemblyName System.Drawing
[S]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Snap([IntPtr]$h, [string]$name) {
    $r = New-Object S+RECT
    [S]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [S]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    $bmp.Save((Join-Path $OutDir $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output "saved $name"
}
function Get-Text([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 512
    [S]::GetWindowTextW($h, $sb, 512) | Out-Null
    return $sb.ToString()
}
function Wait-Window($p) {
    $dl = [DateTime]::Now.AddSeconds(10)
    while ([DateTime]::Now -lt $dl) {
        if ($p.MainWindowHandle -ne 0) { return $p.MainWindowHandle }
        Start-Sleep -Milliseconds 150
    }
    return [IntPtr]::Zero
}
function Wait-Dialog([int]$seconds) {
    $dl = [DateTime]::Now.AddSeconds($seconds)
    while ([DateTime]::Now -lt $dl) {
        $h = [S]::FindWindowW('#32770', [NullString]::Value)
        if ($h -ne [IntPtr]::Zero) { return $h }
        Start-Sleep -Milliseconds 150
    }
    return [IntPtr]::Zero
}
function Stop-Apps {
    Get-Process | Where-Object { $_.ProcessName -like '*winget-picker*' -or $_.ProcessName -like 'wp-test*' } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

$calls = Join-Path $FakeDir 'calls.log'
$env:WINGET_PICKER_LANG = 'zh'
$env:WINGET_PICKER_THEME = 'dark'
$env:WINGET_PICKER_NO_SAVE = '1'
$OrigPath = $env:PATH
$env:PATH = "$FakeDir;$env:PATH"

# ---------------- 1) 静默开 ----------------
Stop-Apps
Remove-Item $calls -ErrorAction SilentlyContinue
$env:WINGET_PICKER_SILENT = '1'
$p = Start-Process -FilePath $Exe -WorkingDirectory $FakeDir -PassThru
$hwnd = Wait-Window $p
if ($hwnd -eq [IntPtr]::Zero) { throw '主窗口未出现（静默开）' }
[S]::SendMessageW($hwnd, 0x111, [IntPtr]1001, [S]::GetDlgItem($hwnd, 1001)) | Out-Null
$status = [S]::GetDlgItem($hwnd, 1009)
$dl = [DateTime]::Now.AddSeconds(30)
while ([DateTime]::Now -lt $dl) {
    Start-Sleep -Milliseconds 250
    if ((Get-Text $status) -match '个可更新软件') { break }
}
[S]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [S]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 300
[S]::PostMessageW($hwnd, 0x111, [IntPtr]1005, [S]::GetDlgItem($hwnd, 1005)) | Out-Null
$confirm = Wait-Dialog 10
if ($confirm -eq [IntPtr]::Zero) { throw '静默升级确认框未出现' }
Start-Sleep -Milliseconds 400
$ctext = ''
for ($i = 0; $i -lt 10; $i++) {
    $ctext = (([S]::ChildTexts($confirm)) -join ' ')
    if ($ctext -match '静默升级') { break }
    Start-Sleep -Milliseconds 200
}
if ($ctext -notmatch '静默升级' -or $ctext -notmatch 'UAC') { throw "确认框文案不符：$ctext" }
Start-Sleep -Milliseconds 400
Snap $confirm 'silent-confirm.png'
[S]::SendMessageW($confirm, 0x111, [IntPtr]6, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 300
$summary = Wait-Dialog 90
if ($summary -eq [IntPtr]::Zero) { throw '升级结束汇总未出现' }
Start-Sleep -Milliseconds 400
$stext = (([S]::ChildTexts($summary)) -join "`n")
Start-Sleep -Milliseconds 700
Snap $summary 'silent-summary.png'
[S]::SendMessageW($summary, 0x111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 400
if (!$p.HasExited) { $p.Kill() }
Stop-Apps

$lines = @(Get-Content $calls -ErrorAction SilentlyContinue)
$silentCalls = @($lines | Where-Object { $_ -match '--id ' -and $_ -match '--silent' }).Count
$plainCalls = @($lines | Where-Object { $_ -match '--id ' -and $_ -notmatch '--silent' }).Count
$gitCalls = @($lines | Where-Object { $_ -match '--id Git\.Git' }).Count
$listCalls = @($lines | Where-Object { $_ -match 'upgrade ' -and $_ -notmatch '--id ' }).Count
"silent_calls=$silentCalls plain_calls=$plainCalls git_calls=$gitCalls list_calls=$listCalls"
if ($silentCalls -ne 47) { throw "静默调用数应为 47，实际 $silentCalls" }
if ($plainCalls -ne 2) { throw "普通回退调用数应为 2（Git/OpenAL），实际 $plainCalls" }
if ($gitCalls -ne 2) { throw "Git.Git 应调用 2 次（静默+普通回退），实际 $gitCalls" }
if ($listCalls -ne 1) { throw "列表调用应为 1 次，实际 $listCalls" }
if ($stext -notmatch '静默升级失败') { throw "汇总缺少静默回退说明：$stext" }
"summary_has_fallback_note=True"
Write-Output "confirm_text_ok=$($ctext -match 'UAC')"

# ---------------- 2) 静默关 ----------------
Stop-Apps
Remove-Item $calls -ErrorAction SilentlyContinue
$env:WINGET_PICKER_SILENT = '0'
$p = Start-Process -FilePath $Exe -WorkingDirectory $FakeDir -PassThru
$hwnd = Wait-Window $p
if ($hwnd -eq [IntPtr]::Zero) { throw '主窗口未出现（静默关）' }
$status = [S]::GetDlgItem($hwnd, 1009)
[S]::SendMessageW($hwnd, 0x111, [IntPtr]1001, [S]::GetDlgItem($hwnd, 1001)) | Out-Null
$dl = [DateTime]::Now.AddSeconds(30)
while ([DateTime]::Now -lt $dl) {
    Start-Sleep -Milliseconds 250
    if ((Get-Text $status) -match '个可更新软件') { break }
}
[S]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [S]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 300
[S]::PostMessageW($hwnd, 0x111, [IntPtr]1005, [S]::GetDlgItem($hwnd, 1005)) | Out-Null
Start-Sleep -Milliseconds 1500
$noConfirm = ([S]::FindWindowW('#32770', [NullString]::Value) -eq [IntPtr]::Zero)
$lines = @(Get-Content $calls -ErrorAction SilentlyContinue)
$silentInOff = @($lines | Where-Object { $_ -match '--silent' }).Count
"silent_off: no_confirm=$noConfirm silent_calls=$silentInOff"
if (!$noConfirm) { throw '静默关闭时不应出现确认框' }
if ($silentInOff -ne 0) { throw '静默关闭时不应带 --silent' }
if ($p.HasExited -eq $false) { $p.Kill() }
Stop-Apps

# ---------------- 3) 设置持久化：点“开”写入 ini ----------------
Remove-Item Env:WINGET_PICKER_SILENT -ErrorAction SilentlyContinue
Remove-Item Env:WINGET_PICKER_NO_SAVE -ErrorAction SilentlyContinue
$env:PATH = $OrigPath
$ini = Join-Path $env:TEMP 'winget-picker-silent-test.ini'
Remove-Item $ini -ErrorAction SilentlyContinue
$env:WINGET_PICKER_INI = $ini
$p = Start-Process -FilePath $Exe -PassThru
$hwnd = Wait-Window $p
if ($hwnd -eq [IntPtr]::Zero) { throw '主窗口未出现（持久化测试）' }
Start-Sleep -Milliseconds 500
[S]::SendMessageW($hwnd, 0x111, [IntPtr]1017, [S]::GetDlgItem($hwnd, 1017)) | Out-Null
Start-Sleep -Milliseconds 500
$iniText = Get-Content $ini -ErrorAction SilentlyContinue | Out-String
"ini_after_silent_on=$([bool]($iniText -match 'silent=1'))"
if ($iniText -notmatch 'silent=1') { throw "ini 未写入 silent=1：$iniText" }
if (!$p.HasExited) { $p.Kill() }
Remove-Item Env:WINGET_PICKER_INI -ErrorAction SilentlyContinue
Stop-Apps
Write-Output 'SILENT_OK'
