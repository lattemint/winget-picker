param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$FakeDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\fake-winget",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens-progress"
)

# 验证“升级过程中”的界面行为：
#   1. 列表仍可滚动；2. 勾选被锁定；3. 当前行自动滚动到可视区并高亮；
#   4. 进度条/状态栏计时正常（截图留证）。
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class U {
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
"@
Add-Type -AssemblyName System.Drawing
[U]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$WM_COMMAND = 0x111
$WM_KEYDOWN = 0x100
$WM_VSCROLL = 0x115
$SB_LINEDOWN = 1
$LVM_GETITEMSTATE = 0x102C
$LVM_GETTOPINDEX = 0x1027
$LVIS_STATEIMAGEMASK = 0xF000

function Get-WindowText([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 512
    [U]::GetWindowTextW($h, $sb, 512) | Out-Null
    return $sb.ToString()
}
function TopIndex([IntPtr]$l) {
    return ([U]::SendMessageW($l, $LVM_GETTOPINDEX, [IntPtr]::Zero, [IntPtr]::Zero)).ToInt64()
}
function ItemChecked([IntPtr]$l, [int]$i) {
    return ([U]::SendMessageW($l, $LVM_GETITEMSTATE, [IntPtr]$i, [IntPtr]$LVIS_STATEIMAGEMASK)).ToInt64() -eq (2 -shl 12)
}
function Shot([IntPtr]$h, [string]$name) {
    $r = New-Object U+RECT
    [U]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [U]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $path = Join-Path $OutDir $name
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Output "saved $path"
}

Get-Process winget-picker,winget -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item "$FakeDir\calls.log" -ErrorAction SilentlyContinue
$OrigPath = $env:PATH
$env:PATH = "$FakeDir;$env:PATH"
$p = Start-Process -FilePath $Exe -WorkingDirectory $FakeDir -PassThru
$hwnd = [IntPtr]::Zero
$deadline = [DateTime]::Now.AddSeconds(10)
while ([DateTime]::Now -lt $deadline) {
    $hwnd = [U]::FindWindowW("WingetPickerWnd", [NullString]::Value)
    if ($hwnd -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 200
}
if ($hwnd -eq [IntPtr]::Zero) { throw "主窗口未出现" }
$list = [U]::GetDlgItem($hwnd, 1010)
$status = [U]::GetDlgItem($hwnd, 1009)

# 打开列表并全选升级
[U]::SendMessageW($hwnd, $WM_COMMAND, [IntPtr]1001, [U]::GetDlgItem($hwnd, 1001)) | Out-Null
$deadline = [DateTime]::Now.AddSeconds(30)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 200
    if ((Get-WindowText $status) -match "个可更新软件") { break }
}
[U]::SendMessageW($hwnd, $WM_COMMAND, [IntPtr]1003, [U]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 300
[U]::SendMessageW($hwnd, $WM_COMMAND, [IntPtr]1005, [U]::GetDlgItem($hwnd, 1005)) | Out-Null
Start-Sleep -Milliseconds 800

# 1) 勾选锁定：按空格尝试取消勾选，应自动还原
$checkedBefore = ItemChecked $list 0
[U]::SendMessageW($list, $WM_KEYDOWN, [IntPtr]32, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 400
$checkedAfter = ItemChecked $list 0
$lockOk = ($checkedBefore -and $checkedAfter)

# 2) 列表可滚动：发送滚动消息后 top index 应变化
$topBefore = TopIndex $list
for ($i = 0; $i -lt 8; $i++) {
    [U]::SendMessageW($list, $WM_VSCROLL, [IntPtr]$SB_LINEDOWN, [IntPtr]::Zero) | Out-Null
}
Start-Sleep -Milliseconds 200
$topAfter = TopIndex $list
$scrollOk = ($topAfter -gt $topBefore)
Write-Output "status=$(Get-WindowText $status)"
Write-Output "lock_ok=$lockOk scroll_ok=$scrollOk (top $topBefore -> $topAfter)"
Shot $hwnd "30_upgrading_scrollable.png"

# 3) 自动跟随：等升级到 30+ 项时，当前行应自动滚入可视区
$topLater = -1
$deadline = [DateTime]::Now.AddSeconds(45)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 250
    $st = Get-WindowText $status
    if ($st -match "正在升级 (3[0-9]|4[0-9])/") {
        $topLater = TopIndex $list
        Write-Output "later_status=$st top=$topLater"
        Shot $hwnd "31_autoscroll_progress.png"
        break
    }
}
$autoscrollOk = ($topLater -gt 0)

# 等结束汇总并回到初始界面
$done = [IntPtr]::Zero
$deadline = [DateTime]::Now.AddSeconds(90)
while ([DateTime]::Now -lt $deadline) {
    Start-Sleep -Milliseconds 300
    $done = [U]::FindWindowW("#32770", "winget 选择性更新")
    if ($done -ne [IntPtr]::Zero) { break }
}
$summaryOk = ($done -ne [IntPtr]::Zero)
if ($summaryOk) {
    [U]::SendMessageW($done, $WM_COMMAND, [IntPtr]1, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 700
}
[U]::PostMessageW($hwnd, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 500
if (!$p.HasExited) { $p.Kill() }
Get-Process winget -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

$calls = (Get-Content "$FakeDir\calls.log" -ErrorAction SilentlyContinue | Measure-Object).Count
Write-Output "summary_ok=$summaryOk calls_total=$calls"
if ($lockOk -and $scrollOk -and $autoscrollOk -and $summaryOk) { Write-Output "ALL_OK" } else { Write-Output "SOME_FAILED" }

if ($OrigPath) { $env:PATH = $OrigPath }
