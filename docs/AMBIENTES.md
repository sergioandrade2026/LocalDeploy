# Ambientes de teste

Web: Docker Linux por padrão, usando Docker Desktop/WSL2 no Windows. Node/Vue/React/Vite/Next e Rust recebem imagem própria por workspace e porta publicada apenas em 127.0.0.1. Servidores Rust/Node precisam escutar 0.0.0.0:3000 no contêiner; PORT=3000 e HOST=0.0.0.0 são fornecidos. Código que ignora essas variáveis precisa de configuração no projeto.

Dioxus: declare application.default_platform no Dioxus.toml. Web usa target wasm32 e CLI dx no contêiner; desktop usa Rust e dx no Windows e abre janela nativa. Bibliotecas/linker nativos precisam estar disponíveis; falhas são registradas no log.

Compose: serviços recebem projeto exclusivo, portas locais dinâmicas e volumes nomeados reservados. Configurações privilegiadas, recursos externos, bind mounts e portas em formato longo são bloqueados para revisão. A limpeza remove contêineres, imagem e volumes do preview. Não remove recursos externos.

Supabase: serviço local exige Compose compatível com isolamento por preview. O app bloqueia a inicialização automática do Supabase CLI sem esse contrato. Não cria credenciais remotas, não presume valores secretos e não aplica migrações de produção. Variáveis vazias declaradas em .env.example são reportadas pelo nome.

Preparar ferramentas e Docker usa winget. Docker Desktop pode exigir habilitar WSL2/virtualização, permissões e reinicialização. Quando Docker não está pronto, a execução informa o bloqueio. Um arquivo localdeploy-native na raiz permite o executor Node nativo para projetos confiáveis; ele não oferece isolamento de sistema.

Isolamento por contêiner não garante execução universal de dependências arbitrárias. Requisitos não reconhecidos são falhas explícitas, sem marcar o build como aprovado.
