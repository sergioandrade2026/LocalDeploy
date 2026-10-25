# LocalDeploy — Especificações funcionais, técnicas e roadmap

**Versão do documento:** 1.0  
**Plataforma-alvo:** Windows 10/11 x86-64  
**Aplicativo:** Rust + Dioxus Desktop 0.7.10 (versão solicitada; validar compatibilidade de crates e CLI no início do desenvolvimento)  
**Princípio:** simples que funciona; zero assinaturas obrigatórias; experiência gráfica sem comandos digitados pelo usuário.

## 1. Objetivo

Criar uma suíte desktop **local-first**, gratuita e portátil, com experiência semelhante ao painel da Vercel: projetos persistentes, histórico de commits, histórico de builds/deploys, botão **Preview** por commit e promoção explícita de builds validados para **Produção local**. GitHub é usado apenas para conectar projetos, sincronizar commits e, opcionalmente, operar um runner GitHub Actions self-hosted. O processamento padrão acontece diretamente na máquina, sem depender de workflows hospedados.

**Definição de custo zero:** nenhuma assinatura, API paga, VPS, domínio ou serviço em nuvem obrigatório. Hardware, energia, internet e eventuais quotas/limites de serviços externos não são gratuitos em sentido absoluto. Não ativar serviços faturáveis automaticamente.

## 2. Experiência do usuário (sem terminal)

1. Usuário baixa um pacote portátil assinado quando possível, extrai e abre `LocalDeploy.exe` com duplo clique. Não exige instalação global para abrir a interface. Pode requerer o runtime WebView2 disponível no Windows; detectar e oferecer instalação guiada se ausente.
2. Tela inicial apresenta **Meus projetos**, com cards, pesquisa, status e botão **Novo projeto**. A URL do GitHub não fica permanentemente na tela inicial.
3. Ao clicar em **Novo projeto**, o usuário cola `https://github.com/organizacao/repositorio`, escolhe branch padrão e pasta local (com padrões inteligentes). A autenticação GitHub abre o navegador do sistema por fluxo OAuth adequado a aplicativo desktop (preferencialmente Device Authorization Flow, exibindo código quando necessário; nunca capturar senha em janela própria). Repositórios públicos podem dispensar login quando a operação permitir.
4. App verifica permissões e clona repositório; exibe um **plano de preparação** com ferramentas, serviços, versões e ações de risco. O usuário confirma alterações relevantes uma vez, não comandos individuais.
5. O projeto aparece na biblioteca, persistindo entre reinicializações. Ao abrir, mostra abas **Visão geral, Commits, Deploys, Ambientes, Dependências, Logs, Configurações**.
6. Em **Commits**, listar SHA abreviado, mensagem, autor, data, branch, status do último build e botões **Testar em localhost**, **Ver logs**, **Promover para produção local** (este último somente quando elegível).
7. Ao clicar **Testar**, preparar o snapshot exato do commit em workspace isolado, verificar mudanças no ambiente, instalar dependências, compilar, testar, iniciar serviços e mostrar URL de preview local. Exibir progresso em etapas, cancelar, tentar novamente e detalhes de erros em linguagem simples.
8. O botão **Produção** promove um artefato já validado para um endpoint local estável (por exemplo `http://localhost:4000`), com confirmação, verificação de saúde e rollback. **Produção local não é hospedagem pública**; acesso pela internet exige configuração separada e consciente de rede, firewall, DNS e TLS.
9. Após sincronização, novos commits aparecem no histórico. Não executar código automaticamente de origem não confiável. Auto-build é opt-in para repositórios confiáveis.

### 2.1 Telas

- **Biblioteca:** cards de projetos, busca, filtro por tecnologia/status, botão Novo projeto, uso de disco e avisos.
- **Novo projeto:** URL, login no navegador, seleção de repositório/branch, pasta, resumo de tecnologias, permissões e botão Criar.
- **Visão geral:** último commit, versão ativa de produção local, preview recente, portas, status dos serviços e atalhos.
- **Commits:** paginação incremental, filtros branch/autor/status, ação de testar cada commit, comparação de alterações de manifesto e lockfile.
- **Deploys:** histórico imutável de tentativas com estados, duração, commit, logs, artefatos, URL e ações de iniciar/parar/limpar/retestar.
- **Ambientes:** Preview e Produção local; variáveis separadas, portas, serviços e estado; não exibir valores secretos.
- **Dependências:** runtime detectado, versão requerida/instalada, origem, integridade, alterações por commit e reparo guiado.
- **Logs:** stream em tempo real, pesquisa, copiar/exportar com mascaramento de segredos.
- **Configurações:** GitHub, cache, armazenamento, recursos, atualização, runner self-hosted opcional, limpeza e remoção.

### 2.2 Estados de build/deploy

`discovered → analyzing → awaiting_approval? → provisioning → installing → building → testing → starting → health_check → ready`.

Saídas alternativas: `failed`, `cancelled`, `blocked`, `stopped`; registrar etapa, erro, duração e ação recomendada. **Build bem-sucedido não significa preview saudável**: confirmar health check antes de exibir `ready`.

## 3. Compatibilidade prioritária

| Tecnologia | Identificação | Preparação e execução |
|---|---|---|
| Vue + Vite + TypeScript | `package.json`, `vite.config.*`, lockfile | Node.js, gerenciador de pacotes correto, `install` reprodutível, scripts de build/test/dev |
| Next.js + React + TypeScript | `next.config.*`, `package.json` | Versão Node compatível, instalação, `next build`, testes configurados, `next start` ou modo dev conforme ambiente |
| Node.js geral | `package.json`, `engines`, `packageManager` | npm/pnpm/yarn/corepack de versão compatível, scripts explicitamente declarados |
| Supabase remoto | `supabase/config.toml`, SDK, `.env.example` | Solicitar URL/chave pública de projeto e segredos de servidor quando exigidos; nunca inferir credenciais |
| Supabase local | arquivos Supabase, migrações | Opcional: Supabase CLI + contêineres/serviços requeridos; detectar Docker/WSL2 e consumo de recursos |
| Rust / Axum | `Cargo.toml`, `rust-toolchain.toml` | rustup/cargo, toolchain e target, `cargo check/test/build`, iniciar binário |
| Dioxus | `Dioxus.toml`, dependências | Rust + CLI dx compatível, target correto (web vs desktop), build/serve conforme projeto |
| Docker Compose | `compose.yaml` ou `docker-compose.yml` | Opcional: engine compatível, volumes, portas, health checks e limites |
| Bancos locais | SQLite/PostgreSQL/Redis | SQLite sem serviço; PostgreSQL/Redis por contêiner quando possível, com dados isolados |

**Regra:** não supor que todo repositório seja executável no Windows. Detectar shell Bash, jobs `ubuntu-latest`, bibliotecas Linux-only, dependências nativas, contêineres e ferramentas de sistema. Exibir incompatibilidade ou oferecer WSL2/Docker mediante aprovação; não prometer equivalência completa com a Vercel ou GitHub Actions Linux.

## 4. Reanálise obrigatória a cada commit

O commit é uma **revisão imutável**: usar SHA completo e checkout isolado (por exemplo `git worktree` em diretório controlado), nunca alterar o workspace de outro preview em execução.

Para cada tentativa:

1. Sincronizar refs de forma segura, resolver SHA e confirmar acesso.
2. Ler novamente árvore de arquivos e manifests, mesmo quando existir cache.
3. Detectar `package.json`, lockfiles (`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`), `.npmrc`, `engines`, `packageManager`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `Dioxus.toml`, `Dockerfile`, Compose, arquivos Supabase, migrações, configs e workflows.
4. Comparar **fingerprint do ambiente** com o último build: SO/arquitetura, SHA, arquivos relevantes, versões de runtimes, ferramentas, flags, configuração de build, imagem/serviços e políticas. Variáveis secretas entram como referência/versão, nunca valor em cache ou logs.
5. Reutilizar apenas caches compatíveis; se o fingerprint mudou, atualizar o plano e instalar componentes ausentes. A mera ausência de alterações em manifests **não garante** equivalência: validar resolução, integridade e saúde do ambiente.
6. Preferir instalação determinística (`npm ci`, `pnpm install --frozen-lockfile`, Yarn imutável, Cargo `--locked` quando adequado). Sem lockfile, avisar sobre reprodutibilidade.
7. Scripts de instalação (`postinstall`, `build.rs`, workflows, migrations) são **código executável**: exigir confiança/aprovação apropriada e isolamento. Nunca elevar privilégios para executar código do repositório.
8. Executar testes disponíveis, build e health checks; quando não houver testes, registrar explicitamente **não configurados**, nunca inventar sucesso.
9. Registrar snapshot de ferramentas, dependências, logs, resultados e artefatos associados ao SHA.

Mudanças em banco de dados requerem cuidado: não aplicar migrações destrutivas à produção local automaticamente; usar banco de preview isolado e pedir aprovação para migrações em produção.

## 5. Preview e Produção local

- **Preview por commit:** URL `http://127.0.0.1:<porta>` alocada pelo gerenciador; porta reservada sem colisão; cada preview tem processo e logs próprios. Permitir previews simultâneos somente dentro dos limites de CPU/RAM/portas; suspensão/expiração de previews antigos.
- **Produção local:** endpoint estável com reverse proxy local opcional, processo supervisionado e persistência de dados definida. Promoção de artefato imutável aprovado; quando build contém configuração específica de ambiente, realizar build de produção separado e validá-lo antes da troca.
- **Rollback:** reativar artefato anterior compatível e configuração correspondente; rollback de banco não é automático sem plano explícito.
- **Variáveis:** escopos preview/produção separados; valores secretos criptografados com DPAPI do Windows ou Credential Manager; nunca incluir segredos no frontend, logs ou artefatos públicos.
- **Rede:** vincular serviços a loopback por padrão. Exposição LAN/internet somente por configuração avançada com avisos, firewall e TLS adequados.

## 6. Arquitetura técnica

- **UI:** Dioxus Desktop 0.7.10, componentes reutilizáveis, tema claro/escuro, acessibilidade, indicadores de estado e ações não bloqueantes.
- **Core Rust:** crates separados `app-ui`, `core`, `github`, `detector`, `toolchains`, `executor`, `preview`, `deploy`, `storage`, `security`, `runner`.
- **Assíncrono:** Tokio para I/O, download, subprocessos e logs; tarefas CPU-bound controladas, filas com concorrência limitada, cancelamento e timeouts.
- **Processos Windows:** `tokio::process::Command` com argumentos separados (evitar concatenação de strings/shell injection). Usar `cmd.exe /C` **somente** quando scripts `.cmd/.bat` exigirem; nunca interpolar entradas não confiáveis em CMD. Para tarefas elevadas, usar mecanismo explícito de UAC com executável auxiliar mínimo e operações permitidas por lista fechada.
- **Git:** Git CLI gerenciado ou biblioteca Rust, com validação de URLs, refs e diretórios; não executar hooks de repositórios não confiáveis.
- **Dados:** SQLite com migrações, WAL quando apropriado, transações e índices. Guardar projetos, branches, commits, builds, deploys, toolchains, ambientes, portas, logs e referências a segredos.
- **Observabilidade:** `tracing`, eventos estruturados, logs por tarefa, sanitização de credenciais e exportação de diagnósticos.
- **Distribuição:** ZIP portátil com `LocalDeploy.exe` e recursos necessários; dados em diretório de usuário configurável, não exigir escrita em `Program Files`; atualizações verificadas e rollback de versão.

### 6.1 Entidades mínimas SQLite

- `projects(id, name, repo_url, repo_owner, repo_name, default_branch, local_path, created_at)`
- `commits(project_id, sha, parent_sha, branch, author, message, committed_at)` (modelar múltiplos pais/branches com tabelas de associação se necessário)
- `builds(id, project_id, commit_sha, environment, fingerprint, status, started_at, finished_at, log_path, artifact_path)`
- `deployments(id, project_id, build_id, target, status, endpoint, deployed_at, superseded_at)`
- `toolchains(id, name, version, path, checksum, source, installed_at)`
- `project_requirements(project_id, commit_sha, kind, name, required_version, detected_source)`
- `environments(id, project_id, name, port, settings_json)`
- `secrets(id, project_id, environment_id, key, protected_reference)`
- `jobs(id, project_id, kind, state, progress_json, created_at, updated_at)`

Usar migrações versionadas, foreign keys, restrições de unicidade e políticas de retenção/limpeza. Segredos nunca entram em `settings_json` em texto puro.

## 7. GitHub e runner opcional

- Autenticar via navegador externo com OAuth Device Flow (quando autorizado para o tipo de app), com escopos mínimos e armazenamento seguro de tokens. Não coletar usuário/senha GitHub.
- Obter lista de branches e commits via API paginada; sincronizar incrementalmente e lidar com rate limits, force-push e repositórios privados.
- Para runners self-hosted: instalar runner oficial Windows x64, solicitar token temporário de registro via API GitHub com permissões apropriadas, atribuir labels, monitorar saúde e permitir remoção/revogação.
- Nunca alterar workflows automaticamente. Se workflow exigir Linux, contêineres ou dependências não suportadas, explicar incompatibilidade e oferecer adaptação **somente com aprovação**.
- Repositórios e PRs não confiáveis não podem executar jobs privilegiados com segredos. Preferir runners efêmeros/isolados e acesso mínimo. Execução local independente do Actions é sempre o caminho padrão.
- O app não garante reprodução idêntica de toda action do GitHub; ele executa seu próprio pipeline local e opcionalmente recebe jobs oficiais do runner.

## 8. Segurança e permissões

1. Tratar repositórios, manifests, scripts e workflows como código não confiável.
2. Nenhuma execução silenciosa de scripts arbitrários imediatamente após colar URL; apresentar plano, origem e solicitações de confiança.
3. Downloads somente de fontes oficiais/confiáveis, via TLS, com checksums/assinaturas quando disponíveis e pinagem de versões.
4. Instalar toolchains no espaço do usuário por padrão. UAC apenas para pré-requisitos realmente globais (ex.: WSL2/serviços), com descrição do impacto.
5. Isolamento por projeto e por commit; limitar CPU, memória, disco, rede e tempo quando a plataforma permitir. Contêineres não são barreira de segurança perfeita; documentar limites.
6. Nunca enviar segredos para o frontend de preview nem para logs; mascaramento e controles de acesso.
7. Operações destrutivas (limpar volumes, migrações, excluir cache, remover runner) requerem confirmação e opções de backup.
8. Não expor localhost publicamente por padrão; evitar uso de portas privilegiadas.
9. Exibir claramente quando uma instalação requer reinicialização e retomar o fluxo depois.

## 9. Regras de simplicidade e custo

- Zero APIs de IA obrigatórias; detecção determinística baseada em manifests, lockfiles e heurísticas auditáveis.
- Não usar servidor central, banco hospedado ou serviço de telemetria obrigatório.
- Não usar GitHub-hosted Actions por padrão; preferir executor local; self-hosted opcional conforme regras vigentes do GitHub.
- Não instalar Docker/WSL2 indiscriminadamente: apenas quando o projeto exigir e o usuário aprovar.
- Limitar caches e previews para não consumir armazenamento indefinidamente.
- Priorizar soluções gratuitas com licenças compatíveis e sem necessidade de conta além do GitHub para repositórios privados.
- Supabase remoto pode ter plano gratuito e limites, mas não é garantidamente ilimitado nem offline; o app não cria serviços pagos automaticamente.

## 10. Roadmap ágil, incremental e testável

Cada tarefa termina com: código completo, `cargo fmt`, `cargo clippy` quando aplicável, testes unitários/integrados, revisão de permissões, logs de erro úteis, verificação de regressão e critérios de aceitação. Não avançar se a etapa estiver quebrada.

### Fase 0 — Contratos e validação de ferramentas
- Confirmar Dioxus 0.7.10 desktop no Windows, compatibilidade de WebView2, CLI e toolchains.
- Criar workspace Rust mínimo, CI de testes do próprio app e arquitetura de crates.
- **Aceite:** executável nativo abre com duplo clique e exibe tela inicial vazia, sem console obrigatório.

### Fase 1 — Biblioteca persistente
- Criar SQLite/migrações, cadastro/remoção de projetos e tela de cards.
- **Aceite:** projeto persiste ao fechar e reabrir, sem duplicações nem perda de dados.

### Fase 2 — GitHub e clonagem
- OAuth no navegador, repositórios públicos/privados, clone seguro, seleção de branch.
- **Aceite:** conectar, cancelar, revogar e clonar; erros de autenticação claros.

### Fase 3 — Histórico de commits
- Listagem paginada, SHA, mensagem, autor, data, branch, atualização incremental.
- **Aceite:** selecionar commit específico e obter snapshot imutável, inclusive após novos commits.

### Fase 4 — Detector de stack
- Parsers de Node/Vue/Next/TypeScript, Rust/Dioxus, Supabase, Docker/Compose.
- **Aceite:** testes com fixtures reais identificam requisitos e casos desconhecidos sem executar código.

### Fase 5 — Preparador de ambiente
- Node/toolchains por projeto, npm/pnpm/yarn, Rust/Cargo/dx; cache, download verificado e UAC guiado quando necessário.
- **Aceite:** ambiente limpo prepara e instala dependências com um clique após aprovação do plano; retoma erros recuperáveis.

### Fase 6 — Executor e reanálise por commit
- Jobs, fingerprint, lockfiles, filas, cancelamento, logs e instalação incremental.
- **Aceite:** adicionar dependência em novo commit provoca atualização antes do build; commit anterior permanece reprodutível.

### Fase 7 — Previews
- Build, testes, portas, processos, health check, URLs localhost, iniciar/parar e múltiplos previews limitados.
- **Aceite:** Vue/Vite e Next.js acessíveis localmente; falha não aparece como sucesso.

### Fase 8 — Supabase e serviços
- Variáveis por ambiente, Supabase remoto, banco de preview isolado, Compose/WSL2 opcionais.
- **Aceite:** fluxo guiado para credenciais, serviços e migrações sem vazar segredos.

### Fase 9 — Produção local e rollback
- Artefatos imutáveis, endpoint estável, promoção, health checks e rollback de aplicação.
- **Aceite:** promover commit validado, manter versão anterior em caso de falha e reverter sem perder dados.

### Fase 10 — Runner GitHub Actions opcional
- Download/registro/health/removal, labels, permissões, compatibilidade de workflow e isolamento.
- **Aceite:** runner autorizado aparece online no GitHub e executa job compatível; remoção revoga registro.

### Fase 11 — Distribuição e endurecimento
- ZIP portátil, logs, diagnósticos, documentação, testes em Windows limpo, atualizações verificadas, limpeza segura.
- **Aceite:** usuário novo baixa, abre, autentica, cadastra projeto e executa preview sem digitar comandos.

## 11. Casos de teste obrigatórios

1. Repositório Vue/Vite/TypeScript com npm e lockfile.
2. Next.js com pnpm e versão Node declarada.
3. Commit A sem dependência X; commit B adiciona X; testar B instala X e testar A continua funcional.
4. Commit altera `packageManager` ou versão de Node; reconfiguração adequada.
5. Supabase remoto sem variáveis; app solicita dados sem expor segredos.
6. Supabase local sem Docker/WSL2; app explica e solicita autorização, sem falhar silenciosamente.
7. Projeto Rust/Axum; build/test/preview; Dioxus desktop não tratado como app web.
8. Repositório privado com token expirado; reautenticação pelo navegador.
9. Script malicioso em `postinstall`; bloqueio/solicitação de confiança e ausência de elevação.
10. Porta ocupada; seleção de nova porta sem matar processos externos.
11. Build interrompido; cancelamento limpa subprocessos e preserva logs.
12. Deploy de produção local falha no health check; versão anterior permanece ativa.
13. Workflow GitHub requer Linux; sinalizar incompatibilidade com runner Windows.
14. Windows sem WebView2/toolchain/SDK; mensagens e instalação guiada, quando possível.
15. Operação offline após clone e cache completo; avisar sobre dependências externas indisponíveis.

## 12. Fora do escopo inicial

- Hospedagem pública com SLA, domínio/TLS automático, CDN e escalonamento global como a Vercel.
- Execução universal de todo workflow GitHub Actions em Windows.
- Provisionamento automático de contas e recursos pagos em provedores externos.
- IA para reescrever código ou corrigir erros silenciosamente.
- Suporte irrestrito a todas as linguagens no MVP.

## 13. Definição de pronto do MVP

Uma pessoa em Windows x86-64 consegue, usando apenas a interface: abrir o executável, autenticar GitHub no navegador, cadastrar projeto, visualizar commits, selecionar commit, reanalisar requisitos, instalar dependências aprovadas, compilar/testar, abrir preview localhost, consultar logs e promover um build saudável para produção local com rollback de aplicação. O fluxo deve funcionar ao menos com Vue/Vite/TypeScript e Next.js/TypeScript; Rust/Axum/Dioxus e Supabase entram nas fases seguintes conforme o roadmap.

---

**Diretriz final:** priorizar correção, isolamento, transparência e tarefas pequenas sobre automação excessiva. O usuário não digita comandos, mas continua no controle de alterações sensíveis e execução de código não confiável.
