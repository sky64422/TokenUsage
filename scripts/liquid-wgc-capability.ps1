param([switch]$CaptureTest)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskOutput = Join-Path $taskRoot 'tmp\liquid-wgc-capability'
$taskLocator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$taskVS = & $taskLocator -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$taskVS) { throw 'MSVC x64 Build Tools are required.' }
$taskDevCmd = Join-Path $taskVS 'Common7\Tools\VsDevCmd.bat'
$taskSource = Join-Path $PSScriptRoot 'liquid-wgc-capability.cpp'
New-Item -ItemType Directory -Force -Path $taskOutput | Out-Null
# Build in tmp: no package installation or changes to the app build configuration.
$taskBuild = "call `"$taskDevCmd`" -arch=x64 -host_arch=x64 >nul`r`ncl /nologo /std:c++17 /EHsc /W4 `"$taskSource`" /Fe:probe.exe /Fo:probe.obj /link windowsapp.lib d3d11.lib user32.lib gdi32.lib`r`nexit /b %errorlevel%`r`n"
$taskBuildPath = Join-Path $taskOutput 'build.cmd'
Set-Content -LiteralPath $taskBuildPath -Value $taskBuild -Encoding ASCII
Push-Location $taskOutput
try {
    & $env:ComSpec /d /c build.cmd
    if ($LASTEXITCODE -ne 0) { throw 'WGC probe compilation failed.' }
    $taskResult = if ($CaptureTest) { & .\probe.exe --capture-test } else { & .\probe.exe }
    $taskExitCode = $LASTEXITCODE
    $taskResult | ConvertFrom-Json | Out-Null
    $taskResultName = if ($CaptureTest) { 'isolation-result.json' } else { 'capability-result.json' }
    $taskResult | Set-Content -LiteralPath $taskResultName -Encoding utf8
    $taskResult
    if ($taskExitCode -ne 0) { throw "WGC probe execution failed: $taskExitCode" }
} finally {
    Pop-Location
}
