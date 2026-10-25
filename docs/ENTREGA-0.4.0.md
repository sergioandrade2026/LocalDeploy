# LocalDeploy 0.4.0

Interface com Deployments, Logs e Configurações separados. Histórico carregado em segundo plano com uma consulta Git para todos os commits exibidos. Subprocessos Windows usam CREATE_NO_WINDOW. Fonte ampliada; preparação de commits mostra carregamento. Previews possuem botão de limpeza e são limpos na próxima abertura do aplicativo, preservando o workspace em produção.

Validação: compilação cruzada Windows x64 e análise estática. Execução completa em Windows ainda precisa de validação.

Atualização: preparação de Git/Node via winget, workflow LocalDeploy gerado pela interface, execução headless por SHA e registro no histórico local, cancelamento das etapas de instalação/build/testes e indicação do comando atual. No Windows, limpeza condicionada à mudança do instante de inicialização do sistema.

Ferramentas: Node portátil selecionado por .nvmrc, .node-version ou engines.node, validado por SHA-256 oficial; Bun instalado via npm. Adaptação de workflows Windows com backup, preservando jobs Linux/contêineres para revisão. O workflow precisa ser commitado e enviado para ativação. Runners previamente registrados precisam receber a label localdeploy-project-ID. A execução completa em Windows não foi validada neste ambiente Linux. O clone mantém objetos Git; somente os workspaces são descartáveis.

O aplicativo ainda não satisfaz todos os critérios do roadmap original.
