param([string]$OutputDirectory = "$PSScriptRoot/../dist/kami-module")
$ErrorActionPreference = 'Stop'
Push-Location "$PSScriptRoot/.."
try {
    cargo build --locked --release --no-default-features --bin localdeploy-engine
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o motor LocalDeploy.' }
    New-Item -ItemType Directory -Force -Path "$OutputDirectory/bin" | Out-Null
    Copy-Item 'target/release/localdeploy-engine.exe' "$OutputDirectory/bin/localdeploy-engine.exe" -Force
    New-Item -ItemType Directory -Force -Path "$OutputDirectory/sdk" | Out-Null
    Copy-Item 'integration/kami-localdeploy/*' "$OutputDirectory/sdk/" -Recurse -Force
    Copy-Item 'integration/PROTOCOL.md' "$OutputDirectory/PROTOCOL.md" -Force
    Copy-Item 'integration/KAMI_ARCHS_INTEGRACAO.md' "$OutputDirectory/INTEGRACAO.md" -Force
    Copy-Item 'integration/localdeploy.module.json' "$OutputDirectory/localdeploy.module.json" -Force
    Write-Output "Módulo preparado em $OutputDirectory. Inclua bin/ nos recursos da IDE e sdk/ no host."
} finally { Pop-Location }
