param(
    [string]$Exe = "C:\Users\MECHREVO\Desktop\winget-picker.exe",
    [string]$OutDir = "C:\Users\MECHREVO\Documents\dsh-workspace\winget-picker\screens-themes"
)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class AB2 {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
}
"@
Add-Type -AssemblyName System.Drawing
[AB2]::SetProcessDPIAware() | Out-Null
function Snap([IntPtr]$h,[string]$name){ $r=New-Object AB2+RECT; [AB2]::GetWindowRect($h,[ref]$r)|Out-Null; $b=New-Object System.Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top); $g=[System.Drawing.Graphics]::FromImage($b); $dc=$g.GetHdc(); [AB2]::PrintWindow($h,$dc,2)|Out-Null; $g.ReleaseHdc($dc); $g.Dispose(); $b.Save((Join-Path $OutDir $name),[System.Drawing.Imaging.ImageFormat]::Png); $b.Dispose(); Write-Output "saved $name" }
function Wait-Window($p){ $dl=[DateTime]::Now.AddSeconds(10); while([DateTime]::Now -lt $dl){ if($p.MainWindowHandle -ne 0){ return $p.MainWindowHandle }; Start-Sleep -Milliseconds 150 }; return [IntPtr]::Zero }
function Wait-About(){ $dl=[DateTime]::Now.AddSeconds(5); while([DateTime]::Now -lt $dl){ $h=[AB2]::FindWindowW('WPAbout',[NullString]::Value); if($h -ne [IntPtr]::Zero){ return $h }; Start-Sleep -Milliseconds 100 }; return [IntPtr]::Zero }
function Stop-Apps { Get-Process | Where-Object { $_.ProcessName -like '*winget*' -or $_.ProcessName -like 'wp-test*' } | Stop-Process -Force -ErrorAction SilentlyContinue }
function Open-About([IntPtr]$main){ [AB2]::PostMessageW($main,0x111,[IntPtr]1014,[AB2]::GetDlgItem($main,1014)) | Out-Null; return (Wait-About) }
Stop-Apps

# 英文/深色：截图 + Esc 关闭
$env:WINGET_PICKER_LANG='en'; $env:WINGET_PICKER_THEME='dark'; $env:WINGET_PICKER_NO_SAVE='1'
$p=Start-Process -FilePath $Exe -PassThru
$main=Wait-Window $p; Start-Sleep -Milliseconds 600
$about=Open-About $main
if ($about -eq [IntPtr]::Zero) { throw '关于窗未出现' }
Start-Sleep -Milliseconds 400
Snap $about 'about-en-dark.png'
[AB2]::PostMessageW($about,0x100,[IntPtr]27,[IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 600
"esc_closed=$(![AB2]::IsWindow($about)) owner_enabled=$([AB2]::IsWindowEnabled($main))"
# Enter 关闭（焦点默认在关闭按钮）
$about=Open-About $main
if ($about -eq [IntPtr]::Zero) { throw '关于窗未出现(2)' }
Start-Sleep -Milliseconds 300
$close=[AB2]::GetDlgItem($about,1040)
"close_btn=$close"
[AB2]::PostMessageW($close,0x100,[IntPtr]13,[IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 600
"enter_closed=$(![AB2]::IsWindow($about)) owner_enabled=$([AB2]::IsWindowEnabled($main))"
if (!$p.HasExited) { $p.Kill() }; Start-Sleep -Milliseconds 300; Stop-Apps

# 中文/浅色
$env:WINGET_PICKER_LANG='zh'; $env:WINGET_PICKER_THEME='light'
$p=Start-Process -FilePath $Exe -PassThru
$main=Wait-Window $p; Start-Sleep -Milliseconds 600
$about=Open-About $main
if ($about -eq [IntPtr]::Zero) { throw '关于窗未出现(zh)' }
Start-Sleep -Milliseconds 400
Snap $about 'about-zh-light.png'
[AB2]::PostMessageW($about,0x111,[IntPtr]1040,[IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 300
if (!$p.HasExited) { $p.Kill() }
Remove-Item Env:WINGET_PICKER_LANG -ErrorAction SilentlyContinue
Remove-Item Env:WINGET_PICKER_THEME -ErrorAction SilentlyContinue
Write-Output 'ABOUT_OK'
