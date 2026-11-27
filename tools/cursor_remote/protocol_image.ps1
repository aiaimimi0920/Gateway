[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Z2-9]{8}$')][string]$Marker,
    [Parameter(Mandatory = $true)][ValidateSet('png', 'jpeg')][string]$Format
)
# Trusted visual fixture only. No model-generated code, OCR, or external image download.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../target/provider-evidence'))
$target = [IO.Path]::GetFullPath($OutputPath)
if (-not $target.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Test-Path -LiteralPath $target)) { throw 'New repository evidence path required' }
$parent = [IO.DirectoryInfo]([IO.Path]::GetDirectoryName($target))
while ($null -ne $parent) {
    if (($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'Linked evidence path rejected'
    }
    $parent = $parent.Parent
}
Add-Type -AssemblyName System.Drawing
$bitmap = New-Object Drawing.Bitmap(720, 180)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
$font = New-Object Drawing.Font('Consolas', 72, [Drawing.FontStyle]::Bold)
try {
    $graphics.Clear([Drawing.Color]::White)
    $graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $graphics.DrawString($Marker, $font, [Drawing.Brushes]::Black, 30, 30)
    $codec = if ($Format -eq 'png') { [Drawing.Imaging.ImageFormat]::Png } else { [Drawing.Imaging.ImageFormat]::Jpeg }
    $bitmap.Save($target, $codec)
} finally {
    $font.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
}
