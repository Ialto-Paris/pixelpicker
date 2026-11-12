# Start-up test of a Pixel Picker executable, run by build.yml and by scripts/check.sh --release:
#   - its version details (file properties) carry the version in Cargo.toml;
#   - it starts, shows its main window within 30 seconds, and exits when that window is closed.
# Usage, from the repository's root: smoke-test.ps1 <path to the executable>
param([Parameter(Mandatory)] [string] $Exe)

$ErrorActionPreference = 'Stop'

$exePath = (Resolve-Path $Exe).Path
$expected = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"$' | Select-Object -First 1).Matches[0].Groups[1].Value
$info = (Get-Item $exePath).VersionInfo
if ($info.ProductVersion -ne $expected -or $info.FileVersion -ne $expected) {
    throw "Version details say $($info.ProductVersion) / $($info.FileVersion), Cargo.toml says $expected"
}
if ($info.CompanyName -ne 'Ialto' -or $info.ProductName -ne 'Pixel Picker') {
    throw "Unexpected version details: $($info.CompanyName) / $($info.ProductName)"
}
Write-Host "Version details: $($info.ProductName) $($info.ProductVersion), $($info.LegalCopyright)"

$process = Start-Process -FilePath $exePath -PassThru
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($process.MainWindowHandle -eq [IntPtr]::Zero) {
        if ($process.HasExited) { throw "Pixel Picker exited at start-up (code $($process.ExitCode))" }
        if ((Get-Date) -gt $deadline) { throw 'Pixel Picker showed no window within 30 seconds' }
        Start-Sleep -Milliseconds 200
        $process.Refresh()
    }
    Write-Host "Main window: '$($process.MainWindowTitle)'"
    if ($process.MainWindowTitle -ne "Pixel Picker $expected") {
        throw "Unexpected window title: '$($process.MainWindowTitle)'"
    }

    [void] $process.CloseMainWindow()
    if (-not $process.WaitForExit(10000)) { throw 'Pixel Picker did not exit when its window was closed' }
    Write-Host 'Started and closed normally.'
}
finally {
    if (-not $process.HasExited) { $process.Kill() }
}
