# make_icon_assets.ps1 -- reproducible icon-asset pipeline for winget-picker.
#
# Renders wingetpicker.svg with Microsoft Edge (headless) so that every icon size
# is rasterized from the vector at its native pixel size (never downscaled from a
# larger render), then assembles:
#
#   assets/app.ico        multi-size ICO, PNG-compressed entries (16..256)
#   assets/logo-256.png   256x256 transparent PNG for the About window
#   assets/app.rc         Win32 resource script: icon id 1 + VERSIONINFO 0.2.0.0
#
# Technique: one temporary HTML page places the SVG seven times, each <img> at its
# exact CSS pixel size, on a transparent background. A single Edge headless run
# screenshots the page at the sheet's exact size (sum of widths x 256), then each
# icon is copied out pixel-for-pixel with System.Drawing. This renders all sizes
# from SVG in one browser session (repeated Edge launches on Windows can stall)
# while still yielding true per-size rasterizations, not resamples. The SVG's own
# white rounded square leaves the outer border transparent, and the full logo and
# text are kept at every size (small sizes blur the text by design).
#
# Usage (from the repository root):
#   pwsh -NoProfile -File tools\make_icon_assets.ps1
#   pwsh -NoProfile -File tools\make_icon_assets.ps1 -KeepTemp
#   pwsh -NoProfile -File tools\make_icon_assets.ps1 -EdgePath "C:\path\to\msedge.exe"
#
# Requirements: Windows 10/11, Microsoft Edge, PowerShell 5.1+ with
# System.Drawing (Windows only). No third-party modules are used.
# Writes only assets/app.ico, assets/logo-256.png, assets/app.rc and a temporary
# render directory that is deleted on exit unless -KeepTemp is set.

[CmdletBinding()]
param(
    [string] $RepoRoot,
    [string] $EdgePath,
    [string] $SvgPath,
    [string] $OutDir,
    [int[]]  $Sizes = @(16, 24, 32, 48, 64, 128, 256),
    [switch] $KeepTemp
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# ---------------------------------------------------------------------------
# Paths and validation
# ---------------------------------------------------------------------------
if (-not $RepoRoot) {
    if ($PSScriptRoot) { $RepoRoot = Split-Path -Parent $PSScriptRoot }
    else { $RepoRoot = (Get-Location).Path }
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
if (-not $SvgPath) { $SvgPath = Join-Path $RepoRoot 'wingetpicker.svg' }
if (-not $OutDir) { $OutDir = Join-Path $RepoRoot 'assets' }
$SvgPath = (Resolve-Path -LiteralPath $SvgPath).Path
if (-not (Test-Path -LiteralPath $OutDir -PathType Container)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$Sizes = @($Sizes | Sort-Object -Unique)
foreach ($size in $Sizes) {
    if ($size -lt 1 -or $size -gt 256) { throw "Icon size $size is out of range (1..256)." }
}
if (-not ($Sizes -contains 256)) {
    throw 'Sizes must include 256: assets/logo-256.png is copied from the 256 px render.'
}

$icoPath = Join-Path $OutDir 'app.ico'
$logoPath = Join-Path $OutDir 'logo-256.png'
$rcPath = Join-Path $OutDir 'app.rc'

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
function Resolve-MsEdge {
    param([string] $Explicit)
    if ($Explicit) {
        if (-not (Test-Path -LiteralPath $Explicit -PathType Leaf)) {
            throw "msedge.exe not found at -EdgePath: $Explicit"
        }
        return (Resolve-Path -LiteralPath $Explicit).Path
    }
    $candidates = New-Object System.Collections.Generic.List[string]
    $bases = @(${env:ProgramFiles(x86)}, $env:ProgramFiles, $env:LOCALAPPDATA)
    foreach ($base in $bases) {
        if ($base) { $candidates.Add((Join-Path $base 'Microsoft\Edge\Application\msedge.exe')) }
    }
    $regKeys = @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe'
    )
    foreach ($regKey in $regKeys) {
        $key = Get-Item -LiteralPath $regKey -ErrorAction SilentlyContinue
        if ($key) {
            $value = $key.GetValue('')
            if ($value) { $candidates.Add([string]$value) }
        }
    }
    $command = Get-Command -Name 'msedge.exe' -ErrorAction SilentlyContinue
    if ($command -and $command.Source) { $candidates.Add([string]$command.Source) }

    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    throw 'Microsoft Edge (msedge.exe) not found. Install Edge or pass -EdgePath <full path>.'
}

# One HTML page with the SVG repeated once per target size, each image laid out at
# exactly that many CSS pixels, laid edge to edge from the left. The page is the
# size of the sheet, so Edge's viewport screenshot maps 1:1 to icon pixels.
function Write-IconSheetHtml {
    param([string] $Path, [string] $SvgFile, [int[]] $SheetSizes)
    $svgUri = ([System.Uri]::new($SvgFile)).AbsoluteUri
    $totalWidth = 0
    $maxHeight = 0
    $images = New-Object System.Collections.Generic.List[string]
    $x = 0
    foreach ($size in $SheetSizes) {
        $images.Add(('<img src="{0}" alt="" style="position:absolute;left:{1}px;top:0;width:{2}px;height:{2}px;">' -f $svgUri, $x, $size))
        $x += $size
        $totalWidth += $size
        if ($size -gt $maxHeight) { $maxHeight = $size }
    }
    $imageMarkup = $images -join "`r`n"
    $html = @"
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>winget-picker icon sheet</title>
<style>
html, body { margin: 0; padding: 0; width: ${totalWidth}px; height: ${maxHeight}px; background: transparent; overflow: hidden; }
</style>
</head>
<body>
$imageMarkup
</body>
</html>
"@
    [System.IO.File]::WriteAllText($Path, $html, (New-Object System.Text.UTF8Encoding($false)))
    return [pscustomobject]@{ TotalWidth = $totalWidth; MaxHeight = $maxHeight }
}

# One headless Edge launch. Output is redirected to files (not pipes) so the call
# cannot block on inherited pipe handles, and the wait is bounded.
function Invoke-EdgeScreenshot {
    param(
        [string] $EdgeExe,
        [string] $HtmlFile,
        [string] $PngFile,
        [int]    $ViewportWidth,
        [int]    $ViewportHeight,
        [string] $ProfileDir,
        [string] $LogDir
    )
    $htmlUri = ([System.Uri]::new($HtmlFile)).AbsoluteUri
    $stdoutFile = Join-Path $LogDir 'edge-stdout.txt'
    $stderrFile = Join-Path $LogDir 'edge-stderr.txt'
    if (Test-Path -LiteralPath $PngFile) { Remove-Item -LiteralPath $PngFile -Force }
    $edgeArgs = @(
        '--headless=new',
        '--disable-gpu',
        '--hide-scrollbars',
        '--force-device-scale-factor=1',
        '--default-background-color=00000000',
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-extensions',
        '--disable-sync',
        '--disable-background-networking',
        '--disable-component-update',
        '--mute-audio',
        "--user-data-dir=$ProfileDir",
        "--window-size=$ViewportWidth,$ViewportHeight",
        "--screenshot=$PngFile",
        $htmlUri
    )
    $process = Start-Process -FilePath $EdgeExe -ArgumentList $edgeArgs -PassThru `
        -WindowStyle Hidden -RedirectStandardOutput $stdoutFile -RedirectStandardError $stderrFile
    $finished = $process.WaitForExit(120000)
    if (-not $finished) {
        try { $process.Kill() } catch { }
        throw "Edge headless timed out after 120 s rendering ${ViewportWidth}x${ViewportHeight}."
    }
    if (-not (Test-Path -LiteralPath $PngFile -PathType Leaf)) {
        $errorText = Get-Content -LiteralPath $stderrFile -Raw -ErrorAction SilentlyContinue
        throw "Edge headless did not create '$PngFile' (exit code $($process.ExitCode)).`n$errorText"
    }
}

# Copy each icon cell out of the sheet pixel-for-pixel (Bitmap.Clone does not
# resample), save it as PNG, and check its dimensions and transparent corner.
function Export-IconCells {
    param([string] $SheetPath, [int[]] $SheetSizes, [string] $CellDir)
    $sheet = [System.Drawing.Bitmap]::new($SheetPath)
    try {
        $expectedWidth = 0
        foreach ($size in $SheetSizes) { $expectedWidth += $size }
        $expectedHeight = ($SheetSizes | Measure-Object -Maximum).Maximum
        if ($sheet.Width -ne $expectedWidth -or $sheet.Height -ne $expectedHeight) {
            throw "Icon sheet is $($sheet.Width)x$($sheet.Height), expected ${expectedWidth}x${expectedHeight}."
        }
        $cells = New-Object System.Collections.Generic.List[object]
        $x = 0
        foreach ($size in $SheetSizes) {
            $rect = [System.Drawing.Rectangle]::new($x, 0, $size, $size)
            $cell = $sheet.Clone($rect, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                if ($cell.Width -ne $size -or $cell.Height -ne $size) {
                    throw "Cell for ${size}px is $($cell.Width)x$($cell.Height)."
                }
                if ($cell.GetPixel(0, 0).A -ne 0) {
                    throw "Cell for ${size}px has an opaque corner; expected a transparent background."
                }
                $cellPath = Join-Path $CellDir ("icon-{0}.png" -f $size)
                $cell.Save($cellPath, [System.Drawing.Imaging.ImageFormat]::Png)
                $cells.Add([pscustomobject]@{ Size = $size; Path = $cellPath })
            }
            finally { $cell.Dispose() }
            $x += $size
        }
        return $cells
    }
    finally { $sheet.Dispose() }
}

# ICONDIR + ICONDIRENTRY + PNG payloads. 256 is stored as 0 in the entry bytes,
# as required by the ICO format; PNG data is used for every size.
function Write-PngIco {
    param([string] $Path, [object[]] $Entries)
    $count = $Entries.Count
    if ($count -lt 1 -or $count -gt 65535) { throw "Invalid ICO entry count: $count" }

    $offset = 6 + 16 * $count
    $stream = [System.IO.MemoryStream]::new()
    $writer = [System.IO.BinaryWriter]::new($stream)
    try {
        $writer.Write([uint16]0)          # reserved
        $writer.Write([uint16]1)          # type: icon
        $writer.Write([uint16]$count)     # image count
        foreach ($entry in $Entries) {
            $size = [int]$entry.Size
            $dimension = if ($size -ge 256) { [byte]0 } else { [byte]$size }
            $writer.Write($dimension)     # bWidth
            $writer.Write($dimension)     # bHeight
            $writer.Write([byte]0)        # bColorCount
            $writer.Write([byte]0)        # bReserved
            $writer.Write([uint16]1)      # wPlanes
            $writer.Write([uint16]32)     # wBitCount
            $writer.Write([uint32]$entry.Bytes.Length)  # dwBytesInRes
            $writer.Write([uint32]$offset)              # dwImageOffset
            $offset += $entry.Bytes.Length
        }
        foreach ($entry in $Entries) { $writer.Write([byte[]]$entry.Bytes) }
        $writer.Flush()
        [System.IO.File]::WriteAllBytes($Path, $stream.ToArray())
    }
    finally {
        $writer.Dispose()
        $stream.Dispose()
    }
}

function Read-BigEndianUInt32 {
    param([byte[]] $Bytes, [int] $Offset)
    return ([uint32]$Bytes[$Offset] -shl 24) -bor
           ([uint32]$Bytes[$Offset + 1] -shl 16) -bor
           ([uint32]$Bytes[$Offset + 2] -shl 8) -bor
           ([uint32]$Bytes[$Offset + 3])
}

# Parse the ICO directory and cross-check every PNG payload (signature + IHDR).
function Test-PngIcoFile {
    param([string] $Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 6) { throw "ICO '$Path' is too small." }
    if ([BitConverter]::ToUInt16($bytes, 0) -ne 0) { throw "ICO '$Path' reserved field is not 0." }
    if ([BitConverter]::ToUInt16($bytes, 2) -ne 1) { throw "ICO '$Path' type field is not 1." }
    $count = [int][BitConverter]::ToUInt16($bytes, 4)
    if ($count -ne $script:expectedIconSizes.Count) {
        throw "ICO '$Path' holds $count entries, expected $($script:expectedIconSizes.Count)."
    }
    $results = @()
    for ($i = 0; $i -lt $count; $i++) {
        $base = 6 + 16 * $i
        $width = [int]$bytes[$base]
        $height = [int]$bytes[$base + 1]
        if ($width -eq 0) { $width = 256 }
        if ($height -eq 0) { $height = 256 }
        if ($width -ne $height) { throw "ICO entry $i is not square ($width x $height)." }
        $dataSize = [int][BitConverter]::ToUInt32($bytes, $base + 8)
        $dataOffset = [int][BitConverter]::ToUInt32($bytes, $base + 12)
        if ($dataOffset + $dataSize -gt $bytes.Length) {
            throw "ICO entry $i payload is out of range."
        }
        if ($dataSize -lt 24 -or
            $bytes[$dataOffset] -ne 0x89 -or $bytes[$dataOffset + 1] -ne 0x50 -or
            $bytes[$dataOffset + 2] -ne 0x4E -or $bytes[$dataOffset + 3] -ne 0x47) {
            throw "ICO entry $i does not start with a PNG signature."
        }
        $pngWidth = [int](Read-BigEndianUInt32 -Bytes $bytes -Offset ($dataOffset + 16))
        $pngHeight = [int](Read-BigEndianUInt32 -Bytes $bytes -Offset ($dataOffset + 20))
        if ($pngWidth -ne $width -or $pngHeight -ne $height) {
            throw "ICO entry $i header says ${width}x${height} but PNG is ${pngWidth}x${pngHeight}."
        }
        if ($width -ne $script:expectedIconSizes[$i]) {
            throw "ICO entry $i is ${width}px, expected $($script:expectedIconSizes[$i])px."
        }
        $results += [pscustomobject]@{
            Index  = $i
            Width  = $width
            Height = $height
            Bytes  = $dataSize
            Offset = $dataOffset
        }
    }
    return $results
}

# ---------------------------------------------------------------------------
# Render and assemble
# ---------------------------------------------------------------------------
$edgeExe = Resolve-MsEdge -Explicit $EdgePath
$edgeVersion = (Get-Item -LiteralPath $edgeExe).VersionInfo.FileVersion

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('wingetpicker-icon-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempRoot | Out-Null
$renderDir = Join-Path $tempRoot 'render'
$profileDir = Join-Path $tempRoot 'edge-profile'
New-Item -ItemType Directory -Path $renderDir | Out-Null

$script:expectedIconSizes = @($Sizes)
$entries = New-Object System.Collections.Generic.List[object]

try {
    Write-Host "Edge: $edgeExe ($edgeVersion)"
    Write-Host "SVG : $SvgPath"
    Write-Host ''

    $sheetHtml = Join-Path $renderDir 'icon-sheet.html'
    $sheetPng = Join-Path $renderDir 'icon-sheet.png'
    $sheet = Write-IconSheetHtml -Path $sheetHtml -SvgFile $SvgPath -SheetSizes $Sizes
    Write-Host ("Rendering {0} icon sizes from SVG on a transparent {1}x{2} sheet ..." -f $Sizes.Count, $sheet.TotalWidth, $sheet.MaxHeight)
    Invoke-EdgeScreenshot -EdgeExe $edgeExe -HtmlFile $sheetHtml -PngFile $sheetPng `
        -ViewportWidth $sheet.TotalWidth -ViewportHeight $sheet.MaxHeight -ProfileDir $profileDir -LogDir $renderDir

    $cells = Export-IconCells -SheetPath $sheetPng -SheetSizes $Sizes -CellDir $renderDir
    Write-Host ''
    foreach ($cell in $cells) {
        $bytes = [System.IO.File]::ReadAllBytes($cell.Path)
        $entries.Add([pscustomobject]@{
            Size  = $cell.Size
            Path  = $cell.Path
            Bytes = $bytes
        })
        Write-Host ("  rendered {0,3}x{0,-3} -> {1,6:N0} bytes PNG" -f $cell.Size, $bytes.Length)
    }

    Write-PngIco -Path $icoPath -Entries $entries.ToArray()

    $entry256 = $entries | Where-Object { $_.Size -eq 256 }
    [System.IO.File]::WriteAllBytes($logoPath, $entry256.Bytes)

    # Windows resource script. ASCII only, so rc.exe needs no code page pragma.
    $rcContent = @'
// app.rc -- winget-picker Windows resources.
// Generated by tools/make_icon_assets.ps1; edit the generator, not this file.

1 ICON "app.ico"

1 VERSIONINFO
 FILEVERSION 0,2,0,0
 PRODUCTVERSION 0,2,0,0
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "LatteMint"
            VALUE "FileDescription", "winget-picker - selective winget upgrade GUI"
            VALUE "FileVersion", "0.2.0.0"
            VALUE "InternalName", "winget-picker"
            VALUE "LegalCopyright", "MIT License (c) 2026 LatteMint"
            VALUE "OriginalFilename", "winget-picker.exe"
            VALUE "ProductName", "winget-picker"
            VALUE "ProductVersion", "0.2.0.0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
'@
    [System.IO.File]::WriteAllText($rcPath, $rcContent, (New-Object System.Text.UTF8Encoding($false)))

    # Self-check: parse the ICO we just wrote and report what Windows will see.
    $icoInfo = Test-PngIcoFile -Path $icoPath
    $sizeList = ($icoInfo | ForEach-Object { '{0}x{1}' -f $_.Width, $_.Height }) -join ', '

    Write-Host ''
    Write-Host "Wrote $icoPath"
    Write-Host "Wrote $logoPath"
    Write-Host "Wrote $rcPath"
    Write-Host ''
    Write-Host "ICO entries ($($icoInfo.Count)): $sizeList"
    foreach ($file in @($icoPath, $logoPath, $rcPath)) {
        $length = (Get-Item -LiteralPath $file).Length
        Write-Host ("  {0,-30} {1,10:N1} KiB" -f (Split-Path -Leaf $file), ($length / 1KB))
    }
}
finally {
    if ($KeepTemp) {
        Write-Host "Temporary render directory kept: $tempRoot"
    }
    else {
        # Edge may hold profile locks for a moment after exiting: retry, then warn.
        for ($attempt = 0; $attempt -lt 15; $attempt++) {
            try {
                Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction Stop
                $tempRoot = $null
                break
            }
            catch {
                Start-Sleep -Milliseconds 300
            }
        }
        if ($tempRoot -and (Test-Path -LiteralPath $tempRoot)) {
            Write-Warning "Could not fully remove temporary directory: $tempRoot"
        }
    }
}
