# Histórico da documentação anterior à extração do módulo

Aplicativo desktop local-first em Rust + Dioxus Desktop 0.7.10, planejado para Windows 10/11 x86-64. A implementação está sendo feita por fases conforme `LOCALDEPLOY_ESPECIFICACOES_ROADMAP.md`.

## Estado desta entrega

Implementado nesta base (Fases 0–1 e partes das fases 2–9):

- Janela desktop Dioxus com painel de biblioteca.
- Cadastro e remoção de registros de projetos.
- Persistência SQLite no diretório de dados local do usuário, com WAL e restrição para não duplicar URL.
- Validação determinística de URLs `https://github.com/owner/repo`.
- Busca local por nome e URL.
- Clone de branch pública ou acessível pelas credenciais do Git já configuradas no sistema.
- Sincronização manual das referências remotas e histórico local de até 50 commits (SHA, assunto, autor e data), com branch atual.
- Tela de detalhes do projeto com seleção de commit e resumo dos requisitos detectados.
- Detecção estática inicial de Node, Vue, Vite, Next, React, TypeScript, Rust/Dioxus, Supabase, Docker Compose, lockfiles e scripts declarados.
- Plano explícito antes de executar código: descreve instalação, build, testes, preview, workspace e riscos; exige confirmação de confiança do usuário.
- Preview por SHA em worktree Git isolado, reanálise do manifesto do próprio SHA, instalação determinística quando há lockfile, build, testes opcionais, porta `127.0.0.1` livre, health check, logs persistidos e ação de parar. Vite é servido pelo artefato `dist` compilado; Next.js usa `start` depois do build.
- Promoção confirmada de um preview saudável para produção local em `http://127.0.0.1:4000`, com pré-validação em porta temporária e rollback para a produção anterior quando disponível.
- Compilação Windows x86-64 validada via cross-compilation; o executável release e o ZIP portátil estão em `dist/LocalDeploy-Windows-x64/` e `dist/LocalDeploy-Windows-x64.zip`.

O clone e a sincronização usam o executável `git` disponível no `PATH`; para repositórios privados, configure credenciais no Git antes de usar o aplicativo. Ele não captura senha nem token. A sincronização busca a branch padrão cadastrada e refs do remoto `origin`; não existe login OAuth nem seletor de branches remotas.

O detector apenas lê manifests e nomes de arquivos. A execução de scripts só acontece depois da confirmação explícita mostrada pelo aplicativo. Toolchains não são instalados automaticamente: Node.js e o gerenciador de pacotes detectado devem existir no `PATH`. Docker, Supabase local, OAuth próprio do GitHub, variáveis secretas criptografadas e publicação na internet ainda não fazem parte desta versão. O painel pode baixar, registrar e iniciar o runner oficial Windows x64 do GitHub Actions para o repositório aberto. O token GitHub com permissão de administração de Actions é usado apenas para solicitar um token temporário de registro e não é salvo pelo aplicativo.

## Requisitos para compilar

- Rust estável e Cargo (versão testada: 1.99.1).
- WebView2 no Windows.
- Bibliotecas do WebKitGTK no Linux, caso seja compilado/testado nesse sistema.
- Git no `PATH` para clonar e sincronizar projetos.
- Node.js e npm, pnpm, Yarn ou Bun no `PATH` para criar previews Node/Vite/Next. O aplicativo informa claramente quando uma ferramenta estiver ausente.

O alvo prioritário é Windows. A compilação cruzada do executável x86-64 passou neste ambiente Linux. O aplicativo não foi executado em Windows; WebView2 continua sendo requisito de execução. Para iniciar uma compilação de release Windows em uma máquina com Rust e toolchain GNU compatível, use `cargo build --release --target x86_64-pc-windows-gnu`.

## Executar em desenvolvimento

Na pasta do projeto, use `cargo run`. O ZIP release inclui `LocalDeploy.exe`, `WebView2Loader.dll`, o WebView2 Fixed Version Runtime e `LEIA-ME.txt`. O aplicativo configura esse runtime a partir da pasta do próprio ZIP, sem instalar WebView2 no Windows. Git for Windows continua sendo necessário para clonar e sincronizar projetos.

Para montar e verificar o pacote Windows após a compilação, execute `scripts/package-windows.sh` e `scripts/verify-windows-package.sh`. Esses scripts falham se a DLL ou outro arquivo essencial estiver ausente.

O runtime incorporado e o procedimento de atualização estão documentados em [docs/WEBVIEW2_RUNTIME.md](docs/WEBVIEW2_RUNTIME.md).

## Persistência

O banco `localdeploy.sqlite3` é criado em `dirs::data_local_dir()/LocalDeploy/`. Nenhum código do repositório é executado ao cadastrar um projeto.

Builds, logs e workspaces isolados ficam nessa mesma pasta de dados. Para uso de teste automatizado, `LOCALDEPLOY_DATA_DIR` pode apontar para uma pasta absoluta diferente.

