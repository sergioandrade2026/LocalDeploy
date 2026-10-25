# Roadmap do produto

Este arquivo acompanha o plano original anexado. O conteúdo integral do roadmap recebido está em `docs/LOCALDEPLOY_ESPECIFICACOES_ROADMAP_ORIGINAL.md`.

## Estado da implementação

- Fase 0: estrutura Dioxus Desktop 0.7.10 criada; `cargo check` e build release Windows x86-64 via cross-compilation validados. ZIP portátil criado. Execução do binário no Windows ainda não validada.
- Fase 1: biblioteca, persistência SQLite, cadastro, busca e remoção implementados.
- Fase 2 (parcial): clone de branch via Git instalado no sistema para URLs GitHub já validadas. Não há OAuth; privados dependem das credenciais previamente configuradas no Git.
- Fase 3 (parcial): busca de refs via `git fetch`, exibição de até 50 commits locais e seleção visual de SHA. Sem API GitHub/paginação/checkout imutável.
- Fase 4 (parcial): detector estático inicial de Node/Vue/Vite/Next/React/TypeScript/Rust-Dioxus/Supabase/Compose e manifests/lockfiles comuns.
- Fase 5 (parcial): verifica a presença do gerenciador de pacotes já instalado; não baixa nem instala toolchains.
- Fase 6 (parcial): worktree isolado por SHA, reanálise dos manifests após checkout, logs por build, estados persistidos e instalação/build/teste após confirmação explícita. Não há fila, cancelamento em andamento ou limite de recursos.
- Fase 7 (parcial): previews Node/Vite/Next com porta loopback, health check, logs e parada do processo. Vite e projetos estáticos são servidos a partir do artefato compilado; Next.js usa o script `start`. Não há monitoramento contínuo nem previews de Rust/Dioxus.
- Fase 9 (parcial): promoção confirmada de preview saudável para loopback `:4000`, pré-validação e rollback do processo da aplicação. Banco de dados e reverse proxy não são migrados automaticamente.
- Fases 8 e 11: não implementadas. Segredos, Supabase guiado, assinatura digital e atualizações verificadas continuam pendentes. Fase 10 é parcial: o aplicativo baixa, registra e inicia o runner oficial no Windows; OAuth, monitoramento pela API e remoção/revogação ainda não estão disponíveis.

O roadmap original continua sendo a fonte de requisitos e critérios de aceitação. Cada fase deve ser validada antes de avançar para a próxima.
