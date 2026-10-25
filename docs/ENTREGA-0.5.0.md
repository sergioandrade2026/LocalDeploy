# LocalDeploy 0.5.0

Novo executor de ambientes: Docker Linux por padrão para aplicações web e Compose, execução nativa para Dioxus desktop. Detecção estática e bloqueios explicados. Veja AMBIENTES.md para contratos e limites.

O botão Preparar ferramentas e Docker instala ferramentas disponíveis via winget. Habilitar virtualização/WSL2, reiniciar o Windows e instalar bibliotecas nativas pode exigir intervenção do usuário. Docker Desktop deve estar iniciado no modo Linux.

Não é reprodução universal de ecossistemas: Rust web precisa respeitar o endereço/porta do executor; Dioxus precisa declarar a plataforma; Supabase local precisa de Compose compatível. Projetos com dependências não cobertas recebem erro e log, não aprovação fictícia.

Validação nesta entrega: 9 testes unitários passaram; teste integrado Docker com instalação, build, testes, resposta HTTP localhost e limpeza passou; compilação/análise estática Windows passou. A janela desktop Windows, WSL2 e Dioxus/Supabase reais não foram executados neste ambiente.
