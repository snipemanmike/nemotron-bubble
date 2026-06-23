[CmdletBinding()]
param(
    [switch]$Multilingual
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

if ($Multilingual) {
    $ModelDir = Join-Path $ProjectRoot "models\nemotron_multi"
    $RepoFolder = "nemotron-3.5-asr-streaming-0.6b-onnx"
} else {
    $ModelDir = Join-Path $ProjectRoot "models\nemotron"
    $RepoFolder = "nemotron-speech-streaming-en-0.6b"
}

$BaseUrl = "https://huggingface.co/altunenes/parakeet-rs/resolve/main/$RepoFolder"
$Files = @(
    "encoder.onnx",
    "encoder.onnx.data",
    "decoder_joint.onnx",
    "tokenizer.model"
)

New-Item -ItemType Directory -Force -Path $ModelDir | Out-Null

foreach ($File in $Files) {
    $Target = Join-Path $ModelDir $File
    if (Test-Path $Target) {
        Write-Host "Already exists: $Target"
        continue
    }

    $Url = "$BaseUrl/$File`?download=true"
    Write-Host "Downloading $File..."
    Invoke-WebRequest -Uri $Url -OutFile $Target -MaximumRedirection 10
}

Write-Host "Nemotron files are ready in $ModelDir"
