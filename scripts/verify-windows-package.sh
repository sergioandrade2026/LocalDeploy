#!/usr/bin/env bash
set -euo pipefail

package_zip="${1:-dist/LocalDeploy-Windows-x64.zip}"
required=(
  'LocalDeploy-Windows-x64/LocalDeploy.exe'
  'LocalDeploy-Windows-x64/WebView2Loader.dll'
  'LocalDeploy-Windows-x64/LEIA-ME.txt'
)

for entry in "${required[@]}"; do
  unzip -Z1 "${package_zip}" | grep -Fxq "${entry}" || {
    echo "Arquivo obrigatório ausente: ${entry}" >&2
    exit 1
  }
done

unzip -Z1 "${package_zip}" | grep -Eq '^LocalDeploy-Windows-x64/WebView2Runtime/.+/msedgewebview2\.exe$' || {
  echo "WebView2 Fixed Version Runtime ausente." >&2
  exit 1
}

echo "Pacote validado: ${package_zip}"
