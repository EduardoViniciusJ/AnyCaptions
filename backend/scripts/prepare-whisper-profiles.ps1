$ErrorActionPreference = 'Stop'
$backendRoot = Split-Path -Parent $PSScriptRoot
$localRoot = Join-Path $backendRoot 'benchmarks/local'
New-Item -ItemType Directory -Force -Path $localRoot | Out-Null

# Synthetic silence only; this script never records the selected audio output.
$silencePath = Join-Path $localRoot 'silence.wav'
$writer = New-Object System.IO.BinaryWriter([System.IO.File]::Create($silencePath))
try {
    $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF'))
    $writer.Write([int]96036)
    $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
    $writer.Write([int]16)
    $writer.Write([int16]1)
    $writer.Write([int16]1)
    $writer.Write([int]16000)
    $writer.Write([int]32000)
    $writer.Write([int16]2)
    $writer.Write([int16]16)
    $writer.Write([Text.Encoding]::ASCII.GetBytes('data'))
    $writer.Write([int]96000)
    $writer.Write((New-Object byte[] 96000))
} finally {
    $writer.Dispose()
}
$silenceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $silencePath).Hash.ToLowerInvariant()
$models = [ordered]@{
    'tiny.en' = '921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f'
    'tiny.en-q5_1' = 'c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b'
    'base.en' = 'a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002'
}
foreach ($model in $models.GetEnumerator()) {
    $config = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $backendRoot 'benchmarks/whisper.example.json') | ConvertFrom-Json
    $config.model.id = $model.Key
    $config.model.path = "../../models/ggml-$($model.Key).bin"
    # Expected hashes are pinned, never inferred from potentially corrupt local files.
    $config.model.sha256 = $model.Value
    $config.fixtures[0].audio.path = 'jfk.wav'
    $config.fixtures += @{
        audio = @{
            id = 'silence'; path = 'silence.wav'; sha256 = $silenceHash
            source = 'synthetic-zero-pcm16'; revision = '1'; license = 'CC0-1.0'
        }
        reference = ''
    }
    $path = Join-Path $localRoot ($model.Key + '.json')
    [IO.File]::WriteAllText($path, ($config | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false)))
}
Write-Output 'Prepared three local profiles and three seconds of synthetic silence. Download the pinned models and JFK fixture separately.'
