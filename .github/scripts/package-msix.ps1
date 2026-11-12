# Builds the Microsoft Store package of the release build:
#   dist/PixelPicker-windows-<arch>.msix   unsigned: the Store signs it after certification
# Run after `cargo build --release`, from the repository's root, on Windows with the Windows
# SDK (MakeAppx and MakePri). The identity comes from packaging/msix/identity.psd1 and the
# version from Cargo.toml; the logos are drawn by examples/msix_assets.rs.
# Its layout, target/msix/layout, can also be installed unpacked for a test, in Developer Mode:
#   Add-AppxPackage -Register target/msix/layout/AppxManifest.xml
param([string] $Architecture = 'x64')

$ErrorActionPreference = 'Stop'

# The newest Windows SDK tool of that name.
function Find-SdkTool([string] $Name) {
    $bin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $tool = Get-ChildItem $bin -Directory -Filter '10.*' |
        Sort-Object { [version] $_.Name } -Descending |
        ForEach-Object { Join-Path $_.FullName "x64\$Name" } |
        Where-Object { Test-Path $_ } |
        Select-Object -First 1
    if (-not $tool) { throw "$Name not found: install the Windows SDK" }
    $tool
}

function Invoke-Tool([string] $Tool, [string[]] $Arguments) {
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$(Split-Path $Tool -Leaf) failed (exit code $LASTEXITCODE)" }
}

$root = (Get-Location).Path
$work = Join-Path $root 'target\msix'
$layout = Join-Path $work 'layout'
$exe = Join-Path $root 'target\release\PixelPicker.exe'
if (-not (Test-Path $exe)) { throw "No release build: run cargo build --release first" }

# The package version has four parts; the Store wants the last one at 0.
$parts = (Select-String -Path Cargo.toml -Pattern '^version = "(\d+)\.(\d+)\.(\d+)"$' |
    Select-Object -First 1).Matches[0].Groups
$version = "$($parts[1].Value).$($parts[2].Value).$($parts[3].Value).0"
$identity = Import-PowerShellDataFile (Join-Path $root 'packaging\msix\identity.psd1')

if (Test-Path $work) { Remove-Item $work -Recurse -Force }
New-Item -ItemType Directory -Force $layout | Out-Null
Copy-Item $exe $layout

$manifest = Get-Content (Join-Path $root 'packaging\msix\AppxManifest.xml') -Raw
$values = [ordered] @{
    '@NAME@'                   = $identity.Name
    '@PUBLISHER_DISPLAY_NAME@' = $identity.PublisherDisplayName
    '@PUBLISHER@'              = $identity.Publisher
    '@VERSION@'                = $version
    '@ARCHITECTURE@'           = $Architecture
}
foreach ($key in $values.Keys) { $manifest = $manifest.Replace($key, $values[$key]) }
if ($manifest -match '@[A-Z_]+@') { throw "Manifest value not filled in: $($Matches[0])" }
[IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest) # UTF-8, no BOM

Invoke-Tool cargo @('run', '--release', '--locked', '--quiet', '--example', 'msix_assets', '--',
    (Join-Path $layout 'Assets'))

# resources.pri maps the manifest's logo names to their scale- and size-qualified files.
$makepri = Find-SdkTool 'makepri.exe'
$priconfig = Join-Path $work 'priconfig.xml'
Invoke-Tool $makepri @('createconfig', '/cf', $priconfig, '/dq', 'en-US', '/pv', '10.0.0', '/o')
# One package, one resources.pri: drop the split into per-scale resource packages, which only
# a bundle would use.
$config = [xml] (Get-Content $priconfig -Raw)
foreach ($node in @($config.SelectNodes('//packaging'))) { [void] $node.ParentNode.RemoveChild($node) }
$config.Save($priconfig)
Invoke-Tool $makepri @('new', '/pr', $layout, '/cf', $priconfig,
    '/mn', (Join-Path $layout 'AppxManifest.xml'), '/of', (Join-Path $layout 'resources.pri'), '/o')

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$package = Join-Path $dist "PixelPicker-windows-$Architecture.msix"
Invoke-Tool (Find-SdkTool 'makeappx.exe') @('pack', '/d', $layout, '/p', $package, '/o')
Write-Host "Package: $package ($((Get-Item $package).Length) bytes), version $version, identity $($identity.Name)"
