# LocalDeploy

Aplicativo desktop para Windows feito em Rust e Dioxus. O pacote portátil inclui o runtime Microsoft WebView2 Fixed Version, então não depende de uma instalação global do WebView2. Extraia a pasta e abra `LocalDeploy.exe`.

## Downloads

Na página [Releases](https://github.com/sergioandrade2026/LocalDeploy/releases), baixe `LocalDeploy-Windows-x64.zip` para usar o aplicativo. Extraia o ZIP inteiro antes de abrir o executável; preserve `WebView2Loader.dll`, `WebView2Runtime/` e `portable.flag` junto dele.

O primeiro pacote para Windows é o build 0.7.0 deste código, com a interface desktop existente. O runtime WebView2 Fixed Version da Microsoft é distribuído dentro do pacote portátil e mantém os avisos de licença fornecidos pela Microsoft. O arquivo tem aproximadamente 301 MB.

## Compilar o aplicativo desktop

Requisitos: Rust stable, Git e toolchain C++ para Windows. Em Windows, o target MSVC é recomendado.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --locked --release --target x86_64-pc-windows-msvc --bin localdeploy
```

O executável resultante fica em `target/x86_64-pc-windows-msvc/release/localdeploy.exe`. Para criar uma pasta portátil que não dependa do WebView2 instalado no sistema, inclua ao lado dele `WebView2Loader.dll`, o runtime Fixed Version x64 da Microsoft em `WebView2Runtime/<versão>/` e `portable.flag`. O pacote de release deste repositório já contém esses itens.

## Motor para integrar com outra IDE

O núcleo também pode ser compilado sem interface gráfica e embutido em outra IDE através do protocolo JSON-RPC por stdin/stdout:

```sh
cargo build --locked --release --no-default-features --bin localdeploy-engine
```

Veja [integration/KAMI_ARCHS_INTEGRACAO.md](integration/KAMI_ARCHS_INTEGRACAO.md) e [integration/PROTOCOL.md](integration/PROTOCOL.md). O SDK JavaScript/TypeScript e o painel Web Component em `integration/kami-localdeploy/` não exigem dependências npm de execução.

## Verificação

```sh
cargo test --locked --no-default-features --lib -- --skip preview_pipeline --skip container_build_and_localhost_preview
cargo clippy --locked --no-default-features --all-targets -- -D warnings
node --test integration/tests/protocol.test.mjs
node --test integration/tests/panel.test.mjs
```

## Limites conhecidos

As tecnologias e serviços suportados e os bloqueios são descritos em [docs/AMBIENTES.md](docs/AMBIENTES.md). O app prepara ambientes locais a partir dos arquivos do projeto; Docker/WSL2 e toolchains de cada ecossistema continuam necessários quando o projeto depende deles. A presença do WebView2 no pacote resolve a interface desktop, não instala runtimes de compilação dos projetos.
