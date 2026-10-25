#!/usr/bin/env bash
set -euo pipefail

target="x86_64-pc-windows-gnu"
release_dir="target/${target}/release"
package_dir="dist/LocalDeploy-Windows-x64"
package_zip="dist/LocalDeploy-Windows-x64.zip"

if [[ ! -f "${release_dir}/localdeploy.exe" ]]; then
  echo "Executável não encontrado: gere-o com cargo build --release --target ${target}." >&2
  exit 1
fi

loader_dll="$(find "target/${target}/release/build" -path '*/out/x64/WebView2Loader.dll' -type f -print -quit)"
if [[ -z "${loader_dll}" ]]; then
  echo "WebView2Loader.dll não foi encontrada no resultado do build." >&2
  exit 1
fi

mkdir -p "${package_dir}"
cp "${release_dir}/localdeploy.exe" "${package_dir}/LocalDeploy.exe"
cp "${loader_dll}" "${package_dir}/WebView2Loader.dll"

if [[ ! -f "${package_dir}/LEIA-ME.txt" ]]; then
  echo "LEIA-ME.txt ausente no pacote." >&2
  exit 1
fi

runtime_executable="$(find "${package_dir}/WebView2Runtime" -name msedgewebview2.exe -type f -print -quit 2>/dev/null || true)"
if [[ -z "${runtime_executable}" ]]; then
  echo "O WebView2 Fixed Version Runtime não foi encontrado no pacote." >&2
  exit 1
fi

rm -f "${package_zip}"
(cd dist && zip -qr -9 "$(basename "${package_zip}")" "$(basename "${package_dir}")")
sha256sum "${package_zip}" "${package_dir}/LocalDeploy.exe" "${package_dir}/WebView2Loader.dll"
