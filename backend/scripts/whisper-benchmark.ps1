param(
    [Parameter(Mandatory = $true)][string]$Experiment,
    [ValidateSet('cpu', 'vulkan')][string]$Backend = 'cpu',
    [ValidatePattern('^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$')][string]$ReportName = 'whisper-result',
    [ValidateRange(30, 3600)][int]$TimeoutSeconds = 600,
    [string]$VulkanSdkPath,
    [string]$VulkanTargetDirectory
)

$ErrorActionPreference = 'Stop'
$backendRoot = Split-Path -Parent $PSScriptRoot
$experimentPath = (Resolve-Path -LiteralPath $Experiment).Path
$feature = if ($Backend -eq 'vulkan') { 'whisper-vulkan' } else { 'whisper-benchmark' }
$savedCmake = $env:CMAKE
$savedBindings = $env:WHISPER_DONT_GENERATE_BINDINGS
$savedTarget = $env:CARGO_TARGET_DIR
$savedCFlags = $env:CMAKE_C_FLAGS_RELEASE
$savedCxxFlags = $env:CMAKE_CXX_FLAGS_RELEASE
$savedVulkanSdk = $env:VULKAN_SDK
$process = $null
$started = $false

try {
    # Keep artifacts isolated from other worktrees; Vulkan's nested shader project
    # also needs a short Windows path to avoid MSBuild path-length failures.
    $env:CARGO_TARGET_DIR = if ($Backend -eq 'vulkan') {
        if ($VulkanTargetDirectory) { (New-Item -ItemType Directory -Force -Path $VulkanTargetDirectory).FullName }
        else { Join-Path (Split-Path -Qualifier $backendRoot) 'ac-vk' }
    } else {
        Join-Path $backendRoot 'target'
    }
    # This MSVC/CMake combination otherwise omitted optimization even in Release.
    $env:CMAKE_C_FLAGS_RELEASE = "$savedCFlags /O2 /DNDEBUG".Trim()
    $env:CMAKE_CXX_FLAGS_RELEASE = "$savedCxxFlags /O2 /DNDEBUG".Trim()
    if (-not $env:CMAKE -and -not (Get-Command cmake -ErrorAction SilentlyContinue)) {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (Test-Path -LiteralPath $vswhere) {
            $installation = & $vswhere -latest -products '*' -property installationPath
            if ($installation) {
                $candidate = Join-Path $installation 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
                if (Test-Path -LiteralPath $candidate) { $env:CMAKE = $candidate }
            }
        }
    }
    if (-not $env:CMAKE -and -not (Get-Command cmake -ErrorAction SilentlyContinue)) {
        throw 'CMake unavailable. Install the Visual Studio CMake component or configure CMAKE.'
    }
    if ($Backend -eq 'vulkan') {
        if ($VulkanSdkPath) {
            $env:VULKAN_SDK = (Resolve-Path -LiteralPath $VulkanSdkPath).Path
        } elseif (-not $env:VULKAN_SDK) {
            $localSdk = Join-Path (Split-Path -Parent $backendRoot) 'backend/local-data/vulkan-sdk-1.4.363.0'
            if (Test-Path -LiteralPath $localSdk) { $env:VULKAN_SDK = $localSdk }
        }
        if (-not $env:VULKAN_SDK -or -not (Test-Path -LiteralPath (Join-Path $env:VULKAN_SDK 'Bin/glslc.exe'))) {
            throw 'Vulkan SDK unavailable. A working Vulkan driver alone is insufficient; configure VULKAN_SDK.'
        }
    }
    if (-not $env:LIBCLANG_PATH -or -not (Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH 'libclang.dll'))) {
        throw 'Configure LIBCLANG_PATH with libclang.dll to generate Windows/MSVC bindings.'
    }
    # The bundled bindings in whisper-rs-sys 0.15.0 have incompatible C layouts on MSVC.
    Remove-Item Env:WHISPER_DONT_GENERATE_BINDINGS -ErrorAction SilentlyContinue
    # Upstream's cmake wrapper disables automatic env tracking for these flags.
    $stampPath = Join-Path $env:CARGO_TARGET_DIR 'whisper-benchmark-native.json'
    $nativeConfig = [ordered]@{
        feature = $feature
        cmake_c_flags_release = $env:CMAKE_C_FLAGS_RELEASE
        cmake_cxx_flags_release = $env:CMAKE_CXX_FLAGS_RELEASE
        cmake = $env:CMAKE
        libclang = $env:LIBCLANG_PATH
        vulkan_sdk = $env:VULKAN_SDK
        cargo_lock = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $backendRoot 'Cargo.lock')).Hash
    } | ConvertTo-Json -Compress
    if (-not (Test-Path -LiteralPath $stampPath) -or (Get-Content -Raw -LiteralPath $stampPath) -ne $nativeConfig) {
    & cargo clean --manifest-path (Join-Path $backendRoot 'Cargo.toml') --release -p whisper-rs-sys
        if ($LASTEXITCODE -ne 0) { throw 'Could not invalidate the native benchmark build.' }
    }
    & cargo build --locked --manifest-path (Join-Path $backendRoot 'Cargo.toml') --release --features $feature --example whisper_benchmark -j 4
    if ($LASTEXITCODE -ne 0) { throw 'Whisper benchmark build failed.' }
    [System.IO.File]::WriteAllText($stampPath, $nativeConfig, (New-Object System.Text.UTF8Encoding($false)))

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo.FileName = Join-Path $env:CARGO_TARGET_DIR 'release/examples/whisper_benchmark.exe'
    $process.StartInfo.Arguments = '"' + $experimentPath + '" ' + $Backend
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    if (-not $process.Start()) { throw 'Could not start benchmark.' }
    $started = $true
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $peakWorkingSet = [long]0
    while (-not $process.WaitForExit(100)) {
        $process.Refresh()
        $peakWorkingSet = [Math]::Max($peakWorkingSet, $process.PeakWorkingSet64)
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Benchmark timed out.' }
    }
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        # Native diagnostics are not persisted; the Rust example returns content-free codes.
        $code = $stderr.Result.Trim()
        if ($code -notmatch '^[a-z_]+$') { $code = 'native_benchmark_failed' }
        throw $code
    }
    $report = $stdout.Result | ConvertFrom-Json
    if ($peakWorkingSet -gt 0) { $report.ram_peak_bytes = $peakWorkingSet }
    $report | Add-Member -NotePropertyName measurement_environment -NotePropertyValue ([ordered]@{
        timestamp_utc = [DateTime]::UtcNow.ToString('o')
        cpu = (Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Name)
        gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
        rustc = (& rustc --version)
        cmake_c_flags_release = $env:CMAKE_C_FLAGS_RELEASE
        cmake_cxx_flags_release = $env:CMAKE_CXX_FLAGS_RELEASE
        cargo_lock_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $backendRoot 'Cargo.lock')).Hash.ToLowerInvariant()
        example_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $backendRoot 'examples/whisper_benchmark/main.rs')).Hash.ToLowerInvariant()
        support_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $backendRoot 'examples/whisper_benchmark/support.rs')).Hash.ToLowerInvariant()
        ram_method = 'Windows process peak working set observed every 100 ms; includes model loading and warm-up'
        vram_method = 'not measured'
        vulkan_sdk = if ($Backend -eq 'vulkan') { (Get-Item -LiteralPath $env:VULKAN_SDK).FullName } else { $null }
        vulkan_glslc = if ($Backend -eq 'vulkan') { (& (Join-Path $env:VULKAN_SDK 'Bin/glslc.exe') --version | Select-Object -First 1) } else { $null }
        game_contention = 'not measured; document a separate controlled game run'
    })
    $outputDirectory = Join-Path $backendRoot 'benchmarks/local'
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $outputPath = Join-Path $outputDirectory ($ReportName + '.json')
    [System.IO.File]::WriteAllText($outputPath, ($report | ConvertTo-Json -Depth 12), (New-Object System.Text.UTF8Encoding($false)))
    Write-Output "Metrics saved: $outputPath"
} finally {
    if ($process) {
        if ($started -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    $env:CMAKE = $savedCmake
    $env:WHISPER_DONT_GENERATE_BINDINGS = $savedBindings
    $env:CARGO_TARGET_DIR = $savedTarget
    $env:CMAKE_C_FLAGS_RELEASE = $savedCFlags
    $env:CMAKE_CXX_FLAGS_RELEASE = $savedCxxFlags
    $env:VULKAN_SDK = $savedVulkanSdk
}
