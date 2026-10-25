#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked --release --no-default-features --bin localdeploy-engine
module_output="${1:-dist/kami-module}"
mkdir -p "$module_output/bin" "$module_output/sdk"
cp target/release/localdeploy-engine "$module_output/bin/"
cp integration/kami-localdeploy/index.mjs integration/kami-localdeploy/index.d.ts integration/kami-localdeploy/panel.mjs integration/kami-localdeploy/package.json integration/kami-localdeploy/README.md "$module_output/sdk/"
cp integration/PROTOCOL.md integration/KAMI_ARCHS_INTEGRACAO.md "$module_output/"
cp integration/localdeploy.module.json "$module_output/"
