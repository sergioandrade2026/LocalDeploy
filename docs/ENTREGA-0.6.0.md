# LocalDeploy 0.6 — uso gráfico e portátil

Extraia a pasta inteira e abra LocalDeploy.exe. O arquivo portable.flag mantém projetos, preferências, logs e dados na pasta Data ao lado do executável. Extraia em uma pasta onde sua conta tenha permissão de escrita. A edição completa inclui WebView2; a leve usa o runtime do Windows.

Fluxo: Conexões → Conectar GitHub → concluir no navegador → Novo projeto → colar URL → Criar e abrir. O clone ocorre em segundo plano. Abra um commit para preparar o teste, acompanhe o progresso e consulte erros em Logs.

Preferências: texto de 14 a 22 px, tema claro/escuro, intervalo de sincronização, testes padrão e execução Docker/nativa. Preparar ambiente instala ferramentas pelo winget e informa bloqueios. Docker/WSL2 e bibliotecas do Windows podem exigir permissões e reinicialização; não são incorporados ao ZIP.

Configurações do projeto: editor gráfico de variáveis, proteção DPAPI e valores ocultos. Os valores são vinculados à conta Windows; ao mudar de usuário/computador, cadastre novamente as credenciais. A biblioteca portátil não leva instalações Docker/WSL2 para outra máquina.

Supabase: o botão abre o painel oficial no navegador. Isso não é OAuth integrado nem importa chaves automaticamente. Cadastre URL/chaves nas variáveis do projeto. Supabase local continua exigindo Compose compatível; o aplicativo apresenta o bloqueio quando não consegue preparar os serviços.

Validação: compilação e análise estática Windows; testes anteriores dos ambientes. Interação gráfica Windows, DPAPI e login real não foram executados neste ambiente Linux.
