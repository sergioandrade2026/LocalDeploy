# Runtime WebView2 incorporado

O pacote Windows inclui o Microsoft Edge WebView2 Fixed Version Runtime x64 na pasta `WebView2Runtime/`. O LocalDeploy define `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` para esse diretório antes de criar a janela, por isso o usuário não precisa instalar o WebView2 Evergreen Runtime.

Versão distribuída: `156.0.4314.42` x64.

Origem oficial:

`https://msedge.sf.dl.delivery.mp.microsoft.com/filestreamingservice/files/074c0930-36ad-4756-9d04-dfe0e503bd94/Microsoft.WebView2.FixedVersionRuntime.156.0.4314.42.x64.cab`

Ao atualizar o runtime, extraia o CAB para `dist/LocalDeploy-Windows-x64/WebView2Runtime/`. A pasta resultante deve conter diretamente uma subpasta com `msedgewebview2.exe`. Depois execute `scripts/package-windows.sh` e `scripts/verify-windows-package.sh`.

O runtime inclui o arquivo de licenças de terceiros fornecido pela Microsoft.
