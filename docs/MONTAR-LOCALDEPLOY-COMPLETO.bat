@echo off
setlocal
set "ARQUIVO=LocalDeploy-Windows-x64-0.2.0.zip"

if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-00" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-01" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-02" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-03" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-04" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-05" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-06" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-07" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-08" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-09" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-10" goto :faltando
if not exist "LocalDeploy-Windows-x64-0.2.0.zip.part-11" goto :faltando

copy /b LocalDeploy-Windows-x64-0.2.0.zip.part-00+LocalDeploy-Windows-x64-0.2.0.zip.part-01+LocalDeploy-Windows-x64-0.2.0.zip.part-02+LocalDeploy-Windows-x64-0.2.0.zip.part-03+LocalDeploy-Windows-x64-0.2.0.zip.part-04+LocalDeploy-Windows-x64-0.2.0.zip.part-05+LocalDeploy-Windows-x64-0.2.0.zip.part-06+LocalDeploy-Windows-x64-0.2.0.zip.part-07+LocalDeploy-Windows-x64-0.2.0.zip.part-08+LocalDeploy-Windows-x64-0.2.0.zip.part-09+LocalDeploy-Windows-x64-0.2.0.zip.part-10+LocalDeploy-Windows-x64-0.2.0.zip.part-11 "%ARQUIVO%" >nul
if errorlevel 1 goto :erro

powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -LiteralPath '%ARQUIVO%' -DestinationPath '.' -Force"
if errorlevel 1 goto :erro

del "%ARQUIVO%"
echo.
echo Pronto. Abra LocalDeploy-Windows-x64\LocalDeploy.exe
pause
exit /b 0

:faltando
echo Faltam partes do pacote. Baixe todas as partes 00 ate 11 para esta mesma pasta.
pause
exit /b 1

:erro
echo Nao foi possivel montar ou extrair o pacote.
pause
exit /b 1
