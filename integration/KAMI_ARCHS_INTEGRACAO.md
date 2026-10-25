# Módulo de testes para Kami Archs IDE

A Kami Archs continua responsável por editar arquivos, Git, login, commits e navegação. LocalDeploy recebe o caminho do repositório e o SHA e cuida de preparação, build/testes, processos, URL e limpeza. Não é preciso abrir a janela desktop do LocalDeploy, instalar WebView2 para o motor ou clonar o projeto novamente.

## Integração recomendada: processo incluído nos recursos da IDE

1. Compile o motor com `cargo build --locked --release --no-default-features --bin localdeploy-engine` no sistema alvo. Os scripts build-kami-module.ps1/.sh montam a pasta de distribuição.
2. Inclua `localdeploy-engine.exe` (Windows) ou `localdeploy-engine` (Linux) nos recursos da IDE. Esse motor não depende de WebView2Loader.dll. O usuário não precisa ter Rust/Cargo para iniciar o motor já compilado.
3. Inclua o adaptador `integration/kami-localdeploy/` no host da IDE, ou use diretamente o protocolo stdio da sua tecnologia.
4. Ao abrir um workspace, anexe seu caminho. O botão Testar chama testCommit; eventos atualizam o painel da IDE, e a URL retornada abre na aba de preview ou no navegador escolhido pela IDE.

```js
import { KamiLocalDeploy } from './localdeploy-sdk/index.mjs';

const engine = await KamiLocalDeploy.start({
  executable: caminhoDoMotorNosRecursosDaIDE,
  dataDirectory: pastaDeDadosDaKamiArchs + '/localdeploy',
});
engine.on('job', atualizarPainelDeTestes);

const preview = await engine.testCommit({
  workspace: caminhoDoWorkspaceAberto,
  branch: branchPrincipal,
  commit: shaSelecionado,
  trusted: workspaceJaAutorizadoPelaIDE,
});
if (preview.endpoint) abrirPreviewNaIDE(preview.endpoint);
```

Somente os nomes da API da Kami Archs precisam ser conectados. Nenhum comando é digitado pelo usuário final. O adaptador Node não tem dependências npm de execução. Não o importe no renderer web: mantenha os subprocessos no host e exponha uma ponte restrita para o painel.

## IDE em Rust/Tauri

Pode usar o mesmo subprocesso, ou consumir a biblioteca:
```toml
[dependencies]
localdeploy = { path = "modules/localdeploy", default-features = false }
```
```rust,no_run
let engine = localdeploy::integration::Engine::open(data_directory)?;
engine.on_event(|event| { /* emitir evento para a janela da IDE */ });
let project = engine.request("projects.attach", serde_json::json!({"path": workspace}))?;
```
Chamadas que consultam Git devem rodar fora da thread gráfica. Builds retornam job_id e trabalham em threads próprias. Uma pasta de dados por processo; use processos separados se precisar de pastas independentes. Em Electron, use o processo principal. Em outro host, escreva/leia JSON por stdin/stdout conforme PROTOCOL.md.

## Interface sugerida

O pacote inclui um Web Component opcional, sem React/Vue ou dependências Node no renderer:
```html
<script type="module" src="./sdk/panel.mjs"></script>
<kami-deployments></kami-deployments>
```
Use `panel.setData({commits, builds, projectName})` e `panel.setJob(job)` para preencher o painel. Ele emite eventos test-commit, open-preview, remove-preview, show-logs, cancel-job e refresh. `integration/examples/panel-bridge.mjs` demonstra a ponte. Cores e fonte herdam variáveis CSS `--kami-background`, `--kami-text`, `--kami-muted`, `--kami-border` e `--kami-font`.

O SDK também exporta `createWorkspaceController(engine, {workspace, branch, isTrusted, openPreview, showLogs})`. Ele fornece todos os métodos usados pela ponte de exemplo; a IDE só precisa mapear sua própria comunicação IPC e seus comandos de abrir URL/logs.

`localdeploy.module.json` descreve os pontos de entrada para seu carregador. Como o formato de plugins da Kami Archs não foi fornecido, ele é um manifesto descritivo, não uma alegação de compatibilidade automática com um carregador existente.

O painel Deployments da IDE mostra mensagem do commit, SHA, resultado e ação. O rótulo Local/Branch/Main vem do Git; Produção deve vir do registro de deploy, não da simples presença na main. O botão só apresenta pronto depois que o job succeeded retornar o build. A URL só existe após previews.open passar no health check. Mostre estados ausente, preparando, aprovado, iniciando, ativo, erro e cancelado. Disponibilize Ver log, Cancelar e Remover teste.

## Escopo e limites reais

- A API foi extraída do aplicativo existente; não torna universais os executores. Veja docs/AMBIENTES.md.
- Docker/WSL2, Rust/linker e serviços de terceiros continuam sendo requisitos externos conforme o projeto. O processo do motor inicia sem eles; seu uso é verificado ao preparar o teste.
- GitHub e Supabase não ganham um novo OAuth por esta integração. A IDE mantém as contas e fornece configurações autorizadas; tokens não precisam atravessar o preview.
- DPAPI vincula variáveis à conta Windows. Em Linux variables.write retorna um bloqueio explícito.
- A IDE deve gerenciar fechamento: cancelar preparação, aguardar tarefas, parar/remover previews conforme sua política e solicitar shutdown. O motor não apaga arquivos editados pela IDE.
- Não há o código da Kami Archs neste pacote; não foi possível alterar/testar sua interface real. O contrato e os adaptadores permitem conectá-la sem depender da tecnologia da IDE.

## Verificação do módulo

```sh
cargo test --locked --no-default-features --lib -- --skip preview_pipeline --skip container_build_and_localhost_preview
cargo build --locked --no-default-features --bin localdeploy-engine
node --test integration/tests/protocol.test.mjs
```

O teste do adaptador cria um repositório temporário, prepara dois commits diferentes, acessa ambos em localhost, verifica que o conteúdo anterior não muda, provoca falha de build, consulta logs e remove os testes. Não exige GitHub, Docker nem a janela desktop, pois usa o executor Node nativo da fixture. Requer Git e Node/npm no ambiente de desenvolvimento.
