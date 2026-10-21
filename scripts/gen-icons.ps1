# Generate the zapret-ui app icon (tray + taskbar) matching the in-app logo:
# a rounded square with a Discord blurple -> YouTube red gradient background
# and white "DY" letters.
# Output: src-tauri/icons/icon-source.png (1024x1024)
# Then run: npm run tauri icon src-tauri/icons/icon-source.png

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root "src-tauri\icons"
$outPath = Join-Path $outDir "icon-source.png"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

$size = 1024
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$g.Clear([System.Drawing.Color]::Transparent)

# Rounded dark background
$radius = 220
$rect = New-Object System.Drawing.Rectangle(0, 0, $size, $size)
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$d = $radius * 2
$path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
$path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
$path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
$path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
$path.CloseFigure()

# Background: gradient blurple -> red, matching the in-app logo (--gradient).
# GDI+ angle is measured clockwise from the x-axis, so 45deg = top-left ->
# bottom-right (same direction as CSS linear-gradient(135deg, ...)).
$bgBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    $rect,
    [System.Drawing.Color]::FromArgb(255, 88, 101, 242),   # Discord blurple #5865F2
    [System.Drawing.Color]::FromArgb(255, 255, 0, 0),       # YouTube red #FF0000
    45.0)
$g.FillPath($bgBrush, $path)
$bgBrush.Dispose()

# "DY" letters in white
$font = New-Object System.Drawing.Font("Segoe UI", 430, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$text = "DY"
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = [System.Drawing.StringAlignment]::Center
$sf.LineAlignment = [System.Drawing.StringAlignment]::Center

$tw = [float]$size
$th = [float]($size - 20)
$textRect = New-Object System.Drawing.RectangleF([float]0, [float]20, $tw, $th)
$white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 255, 255))
$g.DrawString($text, $font, $white, $textRect, $sf)

$white.Dispose()
$sf.Dispose()
$font.Dispose()
$g.Dispose()
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

Write-Host "Icon source saved: $outPath"
