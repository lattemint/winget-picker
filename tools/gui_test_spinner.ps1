param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$FakeDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\fake-winget",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens-themes"
)

# Spinner 回归：
#   1) 加载列表（假 winget 延迟 2s）：spinner 可见且两帧之间有变化
#   2) 列表就绪：spinner 隐藏
#   3) 升级中：spinner 可见且动画变化
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class SP {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
}
"@
Add-Type -AssemblyName System.Drawing
[SP]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Wait-Window($p) {
    $dl = [DateTime]::Now.AddSeconds(10)
    while ([DateTime]::Now -lt $dl) {
        if ($p.MainWindowHandle -ne 0) { return $p.MainWindowHandle }
        Start-Sleep -Milliseconds 150
    }
    return [IntPtr]::Zero
}
function Get-Text([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 512
    [SP]::GetWindowTextW($h, $sb, 512) | Out-Null
    return $sb.ToString()
}
function Capture([IntPtr]$h) {
    $r = New-Object SP+RECT
    [SP]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [SP]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    return $bmp
}
function Get-SpinnerRect([IntPtr]$h) {
    $wr = New-Object SP+RECT
    $cr = New-Object SP+RECT
    [SP]::GetWindowRect($h, [ref]$wr) | Out-Null
    [SP]::GetClientRect($h, [ref]$cr) | Out-Null
    $pt = New-Object SP+POINT
    $pt.X = 0; $pt.Y = 0
    [SP]::ClientToScreen($h, [ref]$pt) | Out-Null
    $scale = $cr.Right / 820.0
    $x = [int](($pt.X - $wr.Left) + 16 * $scale)
    $y = [int](($pt.Y - $wr.Top) + 13 * $scale)
    $w = [int]([Math]::Max(6, 16 * $scale))
    return @($x, $y, $w)
}
function Count-NonBg([System.Drawing.Bitmap]$bmp, $rect) {
    $x = $rect[0]; $y = $rect[1]; $w = $rect[2]
    $count = 0
    for ($dy = 0; $dy -lt $w; $dy++) {
        for ($dx = 0; $dx -lt $w; $dx++) {
            $px = $x + $dx; $py = $y + $dy
            if ($px -lt 0 -or $py -lt 0 -or $px -ge $bmp.Width -or $py -ge $bmp.Height) { continue }
            $c = $bmp.GetPixel($px, $py)
            if ([Math]::Abs($c.R - 245) -gt 8 -or [Math]::Abs($c.G - 246) -gt 8 -or [Math]::Abs($c.B - 248) -gt 8) { $count++ }
        }
    }
    return $count
}
function Diff-Between([System.Drawing.Bitmap]$a, [System.Drawing.Bitmap]$b, $rect) {
    $x = $rect[0]; $y = $rect[1]; $w = $rect[2]
    $count = 0
    for ($dy = 0; $dy -lt $w; $dy++) {
        for ($dx = 0; $dx -lt $w; $dx++) {
            $px = $x + $dx; $py = $y + $dy
            if ($px -lt 0 -or $py -lt 0 -or $px -ge $a.Width -or $py -ge $a.Height) { continue }
            $ca = $a.GetPixel($px, $py); $cb = $b.GetPixel($px, $py)
            if ([Math]::Abs($ca.R - $cb.R) -gt 8 -or [Math]::Abs($ca.G - $cb.G) -gt 8 -or [Math]::Abs($ca.B - $cb.B) -gt 8) { $count++ }
        }
    }
    return $count
}
function Save-Zoom([System.Drawing.Bitmap]$bmp, [string]$path, $rect, [int]$zoom) {
    $x = [Math]::Max(0, $rect[0] - 4); $y = [Math]::Max(0, $rect[1] - 3)
    $w = [Math]::Min($bmp.Width - $x, $rect[2] + 8 + 360)
    $h = [Math]::Min($bmp.Height - $y, $rect[2] + 6)
    $crop = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($crop)
    $g.DrawImage($bmp, (New-Object System.Drawing.Rectangle 0, 0, $w, $h), (New-Object System.Drawing.Rectangle $x, $y, $w, $h), [System.Drawing.GraphicsUnit]::Pixel)
    $g.Dispose()
    $z = New-Object System.Drawing.Bitmap ($w * $zoom), ($h * $zoom)
    $gz = [System.Drawing.Graphics]::FromImage($z)
    $gz.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
    $gz.DrawImage($crop, 0, 0, $w * $zoom, $h * $zoom)
    $gz.Dispose()
    $crop.Dispose()
    $z.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $z.Dispose()
}

Get-Process | Where-Object { $_.ProcessName -like '*winget-picker*' -or $_.ProcessName -like 'wp-test*' } |
    Stop-Process -Force -ErrorAction SilentlyContinue
$env:WINGET_PICKER_THEME = 'light'
$env:WINGET_PICKER_LANG = 'zh'
$env:WINGET_PICKER_NO_SAVE = '1'
$env:WINGET_PICKER_SILENT = '0'
$env:FAKE_WINGET_LIST_DELAY_MS = '2000'
$OrigPath = $env:PATH
$env:PATH = "$FakeDir;$env:PATH"
$p = Start-Process -FilePath $Exe -WorkingDirectory $FakeDir -PassThru
$hwnd = Wait-Window $p
if ($hwnd -eq [IntPtr]::Zero) { throw '主窗口未出现' }
$status = [SP]::GetDlgItem($hwnd, 1009)
[SP]::SendMessageW($hwnd, 0x111, [IntPtr]1001, [SP]::GetDlgItem($hwnd, 1001)) | Out-Null
Start-Sleep -Milliseconds 450
$rect = Get-SpinnerRect $hwnd
"spinner_rect=$($rect -join ',')"
$b1 = Capture $hwnd
Start-Sleep -Milliseconds 160
$b2 = Capture $hwnd
$loadingNonBg = Count-NonBg $b1 $rect
$loadingDiff = Diff-Between $b1 $b2 $rect
"loading_status=[$(Get-Text $status)] loading_nonbg=$loadingNonBg loading_diff=$loadingDiff"
Save-Zoom $b1 (Join-Path $OutDir 'spinner-loading.png') $rect 4
$b1.Dispose(); $b2.Dispose()
if ($loadingNonBg -le 3) { throw '加载阶段 spinner 不可见' }
if ($loadingDiff -le 1) { throw '加载阶段 spinner 没有动画' }

# 等列表就绪
$dl = [DateTime]::Now.AddSeconds(20)
while ([DateTime]::Now -lt $dl) {
    Start-Sleep -Milliseconds 200
    if ((Get-Text $status) -match '个可更新软件') { break }
}
Start-Sleep -Milliseconds 350
$b3 = Capture $hwnd
$spinnerHidden = Count-NonBg $b3 $rect
"list_status=[$(Get-Text $status)] spinner_nonbg_after_list=$spinnerHidden"
$b3.Dispose()
if ($spinnerHidden -gt 2) { throw '列表就绪后 spinner 应隐藏' }

# 升级中
[SP]::SendMessageW($hwnd, 0x111, [IntPtr]1003, [SP]::GetDlgItem($hwnd, 1003)) | Out-Null
Start-Sleep -Milliseconds 200
[SP]::PostMessageW($hwnd, 0x111, [IntPtr]1005, [SP]::GetDlgItem($hwnd, 1005)) | Out-Null
Start-Sleep -Milliseconds 900
$u1 = Capture $hwnd
Start-Sleep -Milliseconds 160
$u2 = Capture $hwnd
$upNonBg = Count-NonBg $u1 $rect
$upDiff = Diff-Between $u1 $u2 $rect
"upgrading_status=[$(Get-Text $status)] upgrading_nonbg=$upNonBg upgrading_diff=$upDiff"
Save-Zoom $u1 (Join-Path $OutDir 'spinner-upgrading.png') $rect 4
$u1.Dispose(); $u2.Dispose()
if ($upNonBg -le 3) { throw '升级中 spinner 不可见' }
if ($upDiff -le 1) { throw '升级中 spinner 没有动画' }

# 等升级结束汇总；关闭后回到初始界面，转圈必须被清除
$summary = [IntPtr]::Zero
$dl = [DateTime]::Now.AddSeconds(60)
while ([DateTime]::Now -lt $dl) {
    $summary = [SP]::FindWindowW('#32770', [NullString]::Value)
    if ($summary -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 250
}
if ($summary -eq [IntPtr]::Zero) { throw '升级结束汇总未出现' }
[SP]::SendMessageW($summary, 0x111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 900
$b4 = Capture $hwnd
$afterNonBg = Count-NonBg $b4 $rect
Save-Zoom $b4 (Join-Path $OutDir 'spinner-after-upgrade.png') $rect 4
$b4.Dispose()
"after_upgrade_status=[$(Get-Text $status)] spinner_nonbg=$afterNonBg"
if ($afterNonBg -gt 2) { throw '升级结束回到初始界面后 spinner 未清除' }

if (!$p.HasExited) { $p.Kill() }
Get-Process | Where-Object { $_.ProcessName -like '*winget-picker*' -or $_.ProcessName -like 'wp-test*' } |
    Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item Env:FAKE_WINGET_LIST_DELAY_MS -ErrorAction SilentlyContinue
$env:PATH = $OrigPath
Remove-Item Env:WINGET_PICKER_THEME -ErrorAction SilentlyContinue
Remove-Item Env:WINGET_PICKER_LANG -ErrorAction SilentlyContinue
Remove-Item Env:WINGET_PICKER_NO_SAVE -ErrorAction SilentlyContinue
Write-Output 'SPINNER_OK'
