# LocalDeploy — estado atual e especificações de uso

Documento de contexto do código publicado em `sergioandrade2026/LocalDeploy`. Registra o comportamento que existe no código e nos pacotes da versão 0.7.0, separa-o das metas antigas e descreve casos de uso para orientar desenvolvimento futuro. Atualizado em 2026-10-25.

## 1. O que é o produto

LocalDeploy é um aplicativo local-first para preparar e testar um commit de um repositório Git em um ambiente local isolado, ver o resultado no navegador ou iniciar um aplicativo Dioxus desktop, consultar logs e, para aplicações web compatíveis, promover um preview para uma produção local em `http://127.0.0.1:4000`.

Há duas formas de uso, com objetivos relacionados, mas interfaces diferentes:

1. **Aplicativo desktop independente:** janela Dioxus Desktop 0.7.10 para Windows. Gerencia biblioteca de projetos, clones GitHub, lista de commits, builds, previews, produção local, configurações, variáveis e runner self-hosted opcional.
2. **Módulo para outra IDE:** biblioteca Rust sem dependência desktop, processo `localdeploy-engine` com protocolo JSON-RPC 2.0 por stdin/stdout, SDK JavaScript/TypeScript e painel Web Component opcional. Foi preparado para a Kami Archs IDE, mas o repositório/código dessa IDE não faz parte deste projeto; a ligação real com sua interface depende do host.

O computador e seus arquivos são o ambiente de execução. O produto não é um serviço de deploy na nuvem e não publica sites na Internet.

## 2. Identidade, repositório e distribuição

- Repositório público: <https://github.com/sergioandrade2026/LocalDeploy>.
- Branch principal: `main`.
- Release: `v0.7.0`, em <https://github.com/sergioandrade2026/LocalDeploy/releases/tag/v0.7.0>.
- Pacote portátil da release: `LocalDeploy-Windows-x64.zip`, com `LocalDeploy.exe`, `WebView2Loader.dll`, `WebView2Runtime/`, `portable.flag` e instruções.
- O pacote da release tem aproximadamente 301 MiB compactado e inclui Microsoft Edge WebView2 Fixed Version Runtime 156.0.4314.42 x64. Deve-se extrair a pasta inteira; não mover o executável sozinho.
- O código é Rust 2021, crate `localdeploy` versão 0.7.0. A interface usa Dioxus Desktop 0.7.10.
- A barra lateral da interface ainda mostra o texto `versão 0.6.0`; esse rótulo visual não foi atualizado junto à versão do crate/release.
- O executável distribuído foi compilado para Windows x64 GNU via cross-compilation neste ambiente. O ZIP passou por verificação estrutural; a interface não foi executada em uma máquina Windows nesta sessão. Builds MSVC feitos por terceiros não foram verificados aqui.
- O runtime contém os arquivos de licença e avisos de terceiros distribuídos pela Microsoft. Não remover esses avisos ao redistribuir o pacote.

## 3. Casos de uso do aplicativo desktop

### 3.1 Primeiro início e preferências

1. Baixar `LocalDeploy-Windows-x64.zip` da página de Releases.
2. Extrair tudo para uma pasta gravável e abrir `LocalDeploy.exe`.
3. O marcador `portable.flag` faz os dados do aplicativo ficarem na pasta `Data` ao lado do executável.
4. Em Preferências, escolher fonte de 14, 16, 18, 20 ou 22 px, tema claro/escuro, intervalo de sincronização de 15 a 300 s, backend Docker/Nativo e execução de testes declarados.
5. O botão de preparação do computador tenta instalar/preparar Git, Node e Docker no Windows. Docker/WSL2 ainda pode exigir permissão administrativa, habilitação de virtualização e reinício.

### 3.2 Conectar GitHub e criar projeto

1. Em **Conexões**, clicar **Conectar GitHub**. O aplicativo usa Git Credential Manager do Git for Windows para iniciar autenticação no navegador; a senha não é solicitada nem armazenada pelo LocalDeploy.
2. Para repositório público, a clonagem pode funcionar sem login. Para repositório privado, o usuário precisa concluir a autenticação e ter acesso a ele.
3. Criar projeto informando URL HTTPS de um repositório GitHub, branch padrão e caminho local. O aplicativo registra o projeto e clona a branch em segundo plano. URL aceita: `https://github.com/owner/repository` (ou `.git`); hosts Git diferentes não são aceitos pelo formulário.
4. A biblioteca permite procurar, abrir e remover projetos cadastrados. Remover o cadastro não deve ser entendido como autorização para apagar o repositório remoto.

O painel Supabase em Conexões apenas abre `https://supabase.com/dashboard`; não implementa OAuth Supabase nem obtém automaticamente chaves do projeto.

### 3.3 Ver commits e sincronizar

1. Abrir um card de projeto: a primeira aba é **Deployments**; outras abas são **Logs** e **Configurações**.
2. A lista mostra até 50 commits recentes disponíveis na cópia local, com mensagem, autor, data, SHA curto e localização.
3. Os commits são obtidos por uma consulta Git sobre as refs locais; `Local` significa não alcançável pelas refs remotas conhecidas, `Branch` é uma ref remota fora da branch padrão e `Main` é alcançável pela branch padrão local/remota.
4. **Sincronizar commits** faz `git fetch --prune origin`. Há sincronização periódica segundo a preferência.
5. O aplicativo não consulta cada commit individualmente pela API GitHub, não busca status GitHub Actions nem pagina todo o histórico remoto. O limite visual é 50 commits.

### 3.4 Preparar e testar um commit

1. Selecionar um commit e abrir **Preparar commit**.
2. Conferir o plano: SHA resolvido, pasta reservada, gerenciador, instalação, build, testes e tipo de preview. As definições dos manifests são reanalisadas após checkout do SHA selecionado.
3. Marcar explicitamente que confia no repositório. Scripts de instalação, build e teste são código executável do projeto.
4. Escolher se executa o script de testes quando existir; a preferência global também define esse padrão.
5. O aplicativo cria um Git worktree para aquele SHA em `Data/workspaces/<project-id>/<sha>`, materializa as variáveis autorizadas, instala dependências, faz build e, se habilitado e declarado, executa testes. A interface exibe progresso e permite cancelar as etapas de processo. Cada etapa tem limite de 30 minutos.
6. Um build só fica `ready` quando as etapas necessárias terminam com sucesso. Se não houver script de testes, isso é registrado como “não configurado”; não é tratado como teste executado.
7. Após sucesso, o preview é iniciado e a aplicação desktop tenta abrir o navegador. URLs web são vinculadas a loopback `127.0.0.1`, em porta livre; uma janela Dioxus desktop inicia como processo nativo sem URL.
8. O projeto original não é substituído pelo worktree. O preview é servido a partir do artefato compilado quando o tipo de aplicação suporta essa estratégia.
9. O usuário pode consultar log, reabrir um preview pronto, parar processos e remover os arquivos temporários de um commit pelo botão `×` na lista.

### 3.5 Produção local e rollback

1. Em **Logs**, selecionar um build `preview` bem-sucedido e clicar **Promover**.
2. A confirmação informa que o destino é local (`127.0.0.1:4000`), não um deploy público.
3. O sistema verifica a versão candidata em uma porta temporária antes de substituir a produção local.
4. Se a candidata passar health check, a aplicação anterior é parada e a nova inicia na porta fixa 4000. Se a nova falhar, há tentativa de reiniciar a versão anterior.
5. A aba Deployments apresenta o botão **Produção** para o SHA ativo. Em Configurações pode-se usar **Reverter produção local** se houver uma produção anterior saudável.

Promover apenas aplica o artefato compatível à produção local. Não faz merge/push para `main`, não configura domínio, TLS, DNS, banco remoto nem hospedagem pública.

### 3.6 Configurar ambiente e variáveis

- A tela Configurações lista tecnologias e requisitos detectados, gerenciador, versão Node, presença de lockfile, scripts e indícios de Supabase/Compose.
- Variáveis são inseridas como `NOME=valor`, uma por linha. No Windows, os valores são protegidos por DPAPI vinculada à conta Windows e aplicados ao workspace isolado do build. Valores são ocultados na edição por padrão; o usuário pode revelá-los temporariamente.
- O conjunto é substituído ao salvar; o conteúdo é reaplicado no próximo build.
- No Linux, a gravação de segredos por DPAPI retorna bloqueio explícito.
- Arquivos `.env.example` e `.env.local.example` são comparados com `.env` para detectar nomes obrigatórios sem valor. Valores de variáveis não devem ser enviados em logs ou à página de preview.

### 3.7 GitHub Actions self-hosted opcional

Na aba Configurações do projeto, o usuário pode:

- gerar `.github/workflows/localdeploy.yml`, que precisa ser revisado, commitado e enviado pelo Git/IDE;
- adaptar alguns jobs Windows compatíveis, com backup e revisão de jobs com container/serviços;
- informar um token com permissão de administração de Actions, confirmar explicitamente o risco, baixar o runner Windows x64 oficial e registrá-lo para aquele repositório;
- iniciar e parar o runner.

O token informado é usado para solicitar token temporário de registro e não é salvo pelo LocalDeploy. Workflows do repositório poderão executar código nesse PC enquanto o runner estiver ativo. Esta função não fornece autenticação OAuth, polling de runs, leitura de logs de Actions ou revogação automática do registro.

## 4. Tecnologias e execução atual

O detector inspeciona manifests e nomes de configuração; detectar uma tecnologia não significa que todas as variantes sejam suportadas.

| Projeto detectado | Ambiente atual | Observações e bloqueios |
|---|---|---|
| Node.js, Vue, React, Vite, Next.js, TypeScript | Docker Linux por padrão; Node nativo opcional nas condições descritas abaixo | Usa npm, pnpm, yarn ou bun. Seleciona scripts e lockfiles conhecidos. Build exige script `build`; os testes usam script `test` se existir. |
| Node.js em modo Nativo | Processo no Windows com permissões do usuário | Selecionar backend Nativo ou adicionar o marcador `localdeploy-native` ao root do repositório. Não é sandbox de segurança. |
| Rust web | Docker Linux | Identificado por `Cargo.toml`; dependências nativas/linker e estrutura de monorepo podem bloquear o build. |
| Dioxus web | Docker Linux | `Dioxus.toml` deve declarar `application.default_platform = "web"`; `dx` pode ser instalado no container. |
| Dioxus desktop | Execução nativa no Windows | Requer Cargo/Rust, Dioxus CLI (`dx`) e toolchain/linker compatíveis. O app testa build e pode iniciar janela nativa; não há URL localhost para desktop. |
| Supabase | Compose fornecido pelo projeto ou variáveis de instância remota | O CLI local não é inicializado automaticamente. Sem um `compose.yaml`/`docker-compose.yml` compatível com serviços e portas isolados, ou `.env` remoto, o build é bloqueado. Não aplicar migrações à produção automaticamente. |
| Docker Compose | Docker Desktop em modo Linux/WSL2 | Configurações privilegiadas, recursos externos, bind mounts e portas longas podem exigir revisão ou serem bloqueadas. Recursos nomeados do preview são limpos no encerramento. |
| Outros ecossistemas | Não há executor genérico | A aplicação retorna motivo de bloqueio; não instala arbitrariamente qualquer dependência de qualquer ecossistema. |

No Windows, o preparo de Node nativo pode baixar uma versão Node compatível com `engines.node`, `.nvmrc` ou `.node-version` em `Data/toolchains`, validar checksum SHA-256 oficial, e preparar gerenciadores conforme o projeto. Isso não substitui toolchains de compilação nem garante compatibilidade universal.

Em Docker, a aplicação web é compilada/servida em contêiner Linux com acesso externo local por loopback. O servidor do projeto precisa aceitar as variáveis/portas fornecidas pelo container. Código que não as respeita pode exigir configuração. Docker/WSL2, virtualização e permissões são dependências do Windows, não componentes incluídos no ZIP LocalDeploy.

## 5. Módulo de integração com Kami Archs IDE

O módulo permite que uma IDE existente seja responsável por edição, clone/login Git, commits e navegação, enquanto LocalDeploy prepara um SHA e gerencia seu ambiente de teste. O fluxo espera que a IDE forneça um workspace Git existente e um commit; ele não clona novamente o projeto.

### Componentes publicados

- `src/lib.rs`: biblioteca Rust e módulos públicos do core.
- `src/integration.rs`: fachada `Engine`, jobs e eventos.
- `src/bin/engine.rs`: servidor JSON-RPC 2.0 de uma mensagem por linha via stdin/stdout. `stdout` é reservado ao protocolo; erros técnicos vão para `stderr`.
- `integration/kami-localdeploy/index.mjs`: cliente Node, start de processo oculto no Windows e helpers de projeto/build/preview.
- `integration/kami-localdeploy/index.d.ts`: tipos TypeScript.
- `integration/kami-localdeploy/panel.mjs`: componente `<kami-deployments>` sem React/Vue e sem dependência npm de runtime.
- `integration/examples/panel-bridge.mjs`: exemplo de ponte entre host e renderer.
- `integration/localdeploy.module.json`: manifesto descritivo com caminhos de recursos e protocolo.
- `integration/PROTOCOL.md`: catálogo de métodos JSON-RPC e estados/eventos.
- `integration/KAMI_ARCHS_INTEGRACAO.md`: instruções de integração.
- `scripts/build-kami-module.ps1` e `.sh`: build do engine e montagem do SDK/documentação.

O formato `localdeploy.module.json` ainda não foi validado contra um carregador/plugin API real da Kami Archs. A interface interna da Kami Archs não está neste repositório. A integração exige que o host inclua o binário, mapeie IPC seguro para o painel e encaminhe abertura de URLs/logs.

### Ciclo de uso do engine

1. Host inicia `localdeploy-engine --data-dir <pasta-absoluta-de-dados>` sem shell interpolation; em Windows usa `windowsHide`/`CREATE_NO_WINDOW`.
2. Host envia `initialize`, anexa o workspace com `projects.attach` e lê `commits.list`.
3. `environment.inspect` e `builds.plan` permitem mostrar os requisitos/avisos antes da aprovação.
4. Com confiança explícita do usuário, chama `builds.prepare` para um SHA. Retorna `job_id`; eventos `localdeploy/event` e `jobs.get` informam progresso e resultado.
5. Após sucesso, chama `previews.open`; abre a URL retornada em aba do IDE ou indica ao host que uma janela desktop foi lançada.
6. `logs.read`, `previews.stop`, `previews.remove`, `jobs.cancel`, settings e variáveis estão disponíveis conforme protocolo.
7. Uma instância do engine controla um diretório de dados. Não abrir engines simultâneos sobre o mesmo banco. Antes de `shutdown`, terminar/cancelar jobs. Previews continuam ativos depois do shutdown até serem parados/removidos ou a máquina reiniciar.

O engine aceita uma tarefa por projeto por vez, mas pode trabalhar com projetos diferentes em paralelo; requisições rápidas continuam disponíveis durante jobs. Builds pesados rodam em workers. Só preparação (`prepare`) aceita cancelamento cooperativo; sincronização, instalação de ambiente, startup/stop de preview não têm cancelamento via API.

O host deve definir a confiança com base no consentimento real do usuário, nunca aceitar `trusted:true` diretamente de conteúdo não confiável do renderer. Conteúdo de projetos pode executar comandos de build/teste com permissões do processo/contêiner e requer aprovação explícita.

## 6. API e estados principais do protocolo

Protocolo detalhado e exemplos estão em [integration/PROTOCOL.md](integration/PROTOCOL.md). Métodos atuais:

`initialize`, `projects.list`, `projects.attach`, `commits.list`, `projects.sync`, `environment.inspect`, `builds.plan`, `builds.prepare`, `builds.list`, `previews.open`, `previews.stop`, `previews.remove`, `jobs.get`, `jobs.cancel`, `logs.read`, `settings.read`, `settings.write`, `variables.write`, `environment.prepare`, `runner.workflow.generate`, `shutdown`.

Estados de job: `running`, `cancelling`, `succeeded`, `failed`, `cancelled`. Estados persistidos de build incluem `ready`, `failed`, `cancelled`, `stopped` e `expired`. Evento assíncrono: `job.updated`, com `id`, `project_id`, `kind`, `status`, `progress`, `result` e `error`. `builds.prepare` retornando `job_id` não significa que o build terminou ou passou.

O engine anexa apenas repositório local Git já existente e testa commits; não recebe mudanças não commitadas do editor. Commits e credenciais remotas são responsabilidade do host/Git instalado. `projects.sync` faz fetch do remote configurado no Git.

## 7. Dados, isolamento, processos e segurança

- Aplicativo independente usa `Data/` ao lado do executável no modo portátil; instalação não portátil usa pasta de dados do sistema definida por `dirs`.
- Biblioteca e builds ficam em SQLite. Preferências, logs, toolchains e worktrees ficam sob a pasta de dados.
- Cada SHA recebe workspace de Git worktree próprio. O projeto-fonte não é apagado por `previews.remove`.
- A limpeza de workspaces de Preview é executada no início. No Windows, o marcador `last-boot` limita a limpeza a uma vez por inicialização do sistema; em outros sistemas ela é tentada a cada início do app. Os registros removidos são marcados `expired`; workspaces da produção local ativa são preservados. A limpeza pode falhar se arquivos/processos estiverem em uso.
- Windows inicia comandos auxiliares sem console visível. Unix e Windows interrompem processos de build e seus filhos/grupos quando cancelado; não há uma janela cmd por commit.
- Previews usam endereços loopback; não prometem serem acessíveis da rede local.
- Scripts de projeto têm execução autorizada pelo usuário e podem realizar ações permitidas pelo sistema. Docker reduz exposição, mas não torna código arbitrário totalmente seguro.
- Segredos Windows via DPAPI estão vinculados à conta. Não incluir pasta `Data`, tokens, `.env`, runner registrado ou dados locais no Git/release.

## 8. Build do próprio LocalDeploy

### Aplicativo desktop no Windows (MSVC recomendado)

Requer Rust stable, Visual Studio Build Tools com Desktop development with C++, Git for Windows. O sistema precisa de WebView2 Evergreen para rodar um build simples, ou usar o pacote Fixed Version na release.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --locked --release --target x86_64-pc-windows-msvc --bin localdeploy
```

Saída MSVC: `target/x86_64-pc-windows-msvc/release/localdeploy.exe`. Incluir apenas esse EXE não cria um pacote portátil: para standalone em PCs sem WebView2 global, incluir `WebView2Loader.dll`, Microsoft WebView2 Fixed Version Runtime em `WebView2Runtime/<versão>/`, `portable.flag` e instruções. O procedimento manual é descrito em [docs/WEBVIEW2_RUNTIME.md](docs/WEBVIEW2_RUNTIME.md); o asset de release é pacote pronto.

### Engine headless para IDE

Sem Dioxus/WebView/GTK:

```powershell
cargo build --locked --release --no-default-features --bin localdeploy-engine
```

Em Unix para development/test do core:

```sh
cargo build --locked --no-default-features --bin localdeploy-engine
cargo clippy --locked --no-default-features --all-targets -- -D warnings
```

### Comandos de validação que já foram executados nesta entrega

```sh
cargo test --locked --no-default-features --lib -- --skip preview_pipeline --skip container_build_and_localhost_preview
node --test integration/tests/protocol.test.mjs
node --test integration/tests/panel.test.mjs
```

Foram observados 9 testes de biblioteca, teste do engine/adaptador com commits distintos, falhas, logs, cancelamento e limpeza, e teste de navegador real do painel com Chromium. Também foi validada a estrutura do ZIP portátil. Esses resultados não equivalem a execução gráfica em Windows ou validação em qualquer repositório/stack real.

## 9. Limitações conhecidas e diferenças entre intenção e implementação

- As fases do roadmap original foram escritas antes da extração do módulo e não representam por si sós o estado atual. O documento original está preservado em `docs/LOCALDEPLOY_ESPECIFICACOES_ROADMAP_ORIGINAL.md`; atualizar este estado junto a mudanças futuras.
- O desktop mostra uma lista Git estilo Deployments, porém o rótulo de ambiente segue `Local`, `Branch` ou `Main` quando não está em produção. Ainda não exibe o rótulo `Preview` que foi pedido como comportamento de Vercel para todo preview pronto.
- A lista desktop tem limite 50; não é um dashboard conectado à API GitHub nem ao status de Actions.
- “Local”/“Branch”/“Main” são classificação de reachability em refs locais após fetch. A semântica não confirma que build remoto passou, que commit foi mergeado por pull request ou publicado em hospedagem.
- Aplicativo standalone clona pela URL GitHub e usa Git instalado/credenciais do usuário. Módulo IDE apenas anexa workspace local já clonado.
- Só stacks detectadas e receitas conhecidas são executadas. Monorepos, manifests em subpastas, arquiteturas nativas arbitrárias e serviços externos podem falhar com blocker.
- Node native é conveniente, não sandbox. Docker é o backend de isolamento para web, sujeito à instalação Linux/WSL2 e à compatibilidade com Docker/Compose.
- Supabase não tem login OAuth/API integrado e migrações não são implantadas; usuário fornece `.env`/Compose.
- Dioxus desktop é build e processo nativo sem URL localhost; não deve ser descrito como preview web.
- Runner self-hosted é opcional e executa workflow code no computador. É necessária permissão de administração de Actions; monitoramento pela API, OAuth automático e revogação não estão implementados.
- Nenhuma atualização automática/verificada, assinatura digital Authenticode, instalador MSIX, release automatizada por CI ou API de plugin da Kami Archs foi validada nesta entrega.
- A interface desktop ainda apresenta “versão 0.6.0” no rodapé apesar do crate/release `0.7.0`; corrigir esse rótulo em uma alteração de interface futura.
- O ZIP portátil atual foi compilado via target GNU; a instrução MSVC no README/spec descreve uma compilação possível no Windows, mas precisa ser validada no ambiente Windows antes de prometer equivalência binária.

## 10. Diretrizes para a próxima instância de desenvolvimento

1. Ler este documento e [README.md](README.md) primeiro; consultar [integration/PROTOCOL.md](integration/PROTOCOL.md) para qualquer alteração de API.
2. Não transformar detecção de tecnologia em alegação de compatibilidade universal: retornar requisitos, progresso, logs e bloqueios reais.
3. Não abrir terminal visível para comandos do produto; processos devem ser criados no backend, sem interpolação shell de dados externos.
4. Manter aprovação explícita antes de executar código do repositório e antes de registrar runners.
5. Preservar dados locais e repositório fonte ao limpar um workspace; testar por SHA com worktree isolado.
6. Não publicar `.env`, DPAPI blobs, runner tokens, dados da pasta `Data` nem `target/` no repositório.
7. Separar mudanças do aplicativo desktop, módulo headless e pacote de distribuição; atualizar este documento, README, versão/rodapé e release notes quando houver mudança de comportamento.
8. Para declarar Windows funcional, testar em Windows real: abrir a UI, login GCM, clone público/privado, build e preview, cancelamento, dados portáteis, WebView2 Fixed Version e Dioxus desktop.

## Referências

- [README](README.md)
- [Integração Kami Archs](integration/KAMI_ARCHS_INTEGRACAO.md)
- [Protocolo JSON-RPC](integration/PROTOCOL.md)
- [Ambientes e compatibilidade](docs/AMBIENTES.md)
- [Runtime WebView2](docs/WEBVIEW2_RUNTIME.md)
- [Roadmap original recebido](docs/LOCALDEPLOY_ESPECIFICACOES_ROADMAP_ORIGINAL.md)
