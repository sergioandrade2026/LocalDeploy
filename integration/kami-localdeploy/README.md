# Adaptador para o host da Kami Archs

Sem dependências npm de execução. Use em Node 18+, no processo principal Electron ou no host de extensões. Não importe diretamente em uma página web; exponha apenas as ações necessárias pela ponte IPC da IDE.

```js
import { KamiLocalDeploy } from './integration/kami-localdeploy/index.mjs';

const tests = await KamiLocalDeploy.start({
  executable: ide.resourcesPath('localdeploy-engine.exe'),
  dataDirectory: ide.dataPath('localdeploy'),
});

tests.on('job', job => ide.tests.update(job));

// A IDE obtém estes valores do workspace e da política de confiança já existente.
const preview = await tests.testCommit({
  workspace: ide.workspace.root,
  branch: ide.workspace.defaultBranch,
  commit: selectedCommit.sha,
  trusted: ide.workspace.isTrusted,
});

if (preview.endpoint) ide.openPreview(preview.endpoint);
```

`ide.*` representa a API da sua IDE; não é uma dependência deste pacote. O motor não clona, abre navegador nem coleta credenciais ao anexar um workspace existente. O SDK oferece eventos e consulta o estado das tarefas para não perder respostas caso um evento chegue antes da inscrição.

`close()` recusa encerrar com tarefas ativas; cancele builds e aguarde a finalização antes. Previews já abertos continuam ativos após desligar o motor, até serem parados/removidos pela IDE. Um AbortSignal cancela a preparação; se a inicialização do preview já começou, aguarde sua conclusão e remova o preview.
