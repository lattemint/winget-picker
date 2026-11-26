param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$OutDir = "C:\Users\MECHREVO\Desktop\winget-picker-shots"
)

$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class V {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
"@ -ErrorAction Stop
Add-Type -AssemblyName System.Drawing
[V]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Get-WindowText([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 1024
    [V]::GetWindowTextW($h, $sb, 1024) | Out-Null
    return $sb.ToString()
}

function Save-WindowShot([IntPtr]$h, [string]$name) {
    $r = New-Object V+RECT
    [V]::GetWindowRect($h, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left
    $hh = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [V]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $path = Join-Path $OutDir $name
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Output "saved $path"
}

$env:WINGET_PICKER_THEME = 'dark'
$env:WINGET_PICKER_LANG = 'zh'
$env:WINGET_PICKER_SILENT = '0'
$env:WINGET_PICKER_NO_SAVE = '1'
Get-Process | Where-Object { $_.ProcessName -like '*winget-picker*' -or $_.ProcessName -like 'wp-test*' } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

$p = Start-Process -FilePath $Exe -PassThru
$hwnd = [IntPtr]::Zero
$deadline = [DateTime]::Now.AddSeconds(10)
while ([DateTime]::Now -lt $deadline) {
    if ($p.MainWindowHandle -ne 0) { $hwnd = $p.MainWindowHandle; break }
    Start-Sleep -Milliseconds 200
}
if ($hwnd -eq [IntPtr]::Zero) { throw "主窗口未出现" }
Write-Output "hwnd=$hwnd pid=$($p.Id)"
Start-Sleep -Milliseconds 500
Save-WindowShot $hwnd "01_initial.png"

# 查看升级
$btnView = [V]::GetDlgItem($hwnd, 1001)
[V]::SendMessageW($hwnd, 0x111, [IntPtr]1001, $btnView) | Out-Null
$status = [V]::GetDlgItem($hwnd, 1009)
$listLoaded = $false
$msg = [IntPtr]::Zero
$deadline = [DateTime]::Now.AddSeconds(60)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 300
    $msg = [V]::FindWindowW("#32770", "winget 选择性更新")
    if ($msg -ne [IntPtr]::Zero) { break }
    if ((Get-WindowText $status) -match "个可更新软件") { $listLoaded = $true; break }
}
Write-Output "list_loaded=$listLoaded msgbox=$msg status=$(Get-WindowText $status)"
Save-WindowShot $hwnd "02_list_real.png"

if ($msg -ne [IntPtr]::Zero) {
    Save-WindowShot $msg "03_msgbox.png"
    [V]::SendMessageW($msg, 0x111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 600
    [V]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 500
    Get-Process winget-picker -ErrorAction SilentlyContinue | Stop-Process -Force
    Write-Output "DONE_WITH_ERROR"
    exit 1
}

# 全选（只验证选择，不执行真实升级）
[V]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [V]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 500
$btnUp = [V]::GetDlgItem($hwnd, 1005)
Write-Output "upgrade_button=$(Get-WindowText $btnUp)"
Save-WindowShot $hwnd "03_all_checked.png"

# 返回初始界面
[V]::SendMessageW($hwnd, 0x111, [IntPtr]1006, [V]::GetDlgItem($hwnd, 1006)) | Out-Null
Start-Sleep -Milliseconds 500
Save-WindowShot $hwnd "04_back_to_init.png"
Write-Output "back_status_text=$(Get-WindowText $status)"

# 关闭
[V]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 700
$exited = $p.HasExited
Write-Output "exited_on_close=$exited"
if (!$exited) { $p.Kill() }
Get-Process winget -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Output "DONE_OK"
