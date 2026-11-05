; Instalador do FeatherDocs para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/featherdocs.exe -DSAIDA=caminho/instalador.exe pacote/featherdocs.nsi
; Instala para todos os usuários em "Arquivos de Programas\FeatherWorks\FeatherDocs" (pede
; administrador uma vez: só o administrador altera os .exe; a pasta FeatherWorks é da suíte), cria atalhos, associa o .fdoz
; e registra o desinstalador em "Aplicativos instalados".

Unicode true
!include "MUI2.nsh"

!ifndef VERSAO
  !define VERSAO "0.0.0"
!endif

Name "FeatherDocs ${VERSAO}"
OutFile "${SAIDA}"
InstallDir "$PROGRAMFILES64\FeatherWorks\FeatherDocs"
InstallDirRegKey HKLM "Software\FeatherWorks\FeatherDocs" "Pasta"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!define MUI_ICON "icones\featherdocs.ico"
!define MUI_UNICON "icones\featherdocs.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\featherdocs.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o FeatherDocs"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

!include "FileFunc.nsh"
!define DESINSTALAR "Software\Microsoft\Windows\CurrentVersion\Uninstall\FeatherDocs"

; Atalhos de todos os usuários e o registro de 64 bits (o programa é de 64 bits).
Function .onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Function un.onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Section "FeatherDocs"
  ; As versões antigas instalavam só para o usuário (%LOCALAPPDATA%\Programs): tira essa cópia.
  ; _?= faz o desinstalador antigo rodar no lugar e esperar; o próprio .exe e a pasta saem aqui.
  ReadRegStr $2 HKCU "Software\FeatherWorks\FeatherDocs" "Pasta"
  StrCmp $2 "" antiga_fora
  IfFileExists "$2\desinstalar.exe" 0 antiga_fora
    ExecWait '"$2\desinstalar.exe" /S _?=$2'
    Delete "$2\desinstalar.exe"
    RMDir "$2"
  antiga_fora:

  SetOutPath "$INSTDIR"
  ; Com o programa aberto, o .exe em uso não se sobrescreve, mas se renomeia; o .old sai
  ; na próxima instalação ou na desinstalação.
  Delete "$INSTDIR\featherdocs.old"
  Rename "$INSTDIR\featherdocs.exe" "$INSTDIR\featherdocs.old"
  File "/oname=featherdocs.exe" "${EXE}"
  File "icones\featherdocs.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  File "/oname=LICENCA-SINONIMOS.txt" "..\crates\featherdocs-pelicano\dados\ortografia\LICENCA-SINONIMOS.txt"
  File "/oname=LICENCA-DICIONARIO-INGLES.txt" "..\crates\featherdocs-pelicano\dados\ortografia\README_en_US.txt"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherDocs.lnk" "$INSTDIR\featherdocs.exe" "" "$INSTDIR\featherdocs.ico"
  CreateShortCut "$DESKTOP\FeatherDocs.lnk" "$INSTDIR\featherdocs.exe" "" "$INSTDIR\featherdocs.ico"

  ; Documento do FeatherDocs (.fdoz): ícone e abrir com dois cliques.
  WriteRegStr HKLM "Software\Classes\.fdoz" "" "FeatherDocs.Documento"
  WriteRegStr HKLM "Software\Classes\FeatherDocs.Documento" "" "Documento do FeatherDocs"
  WriteRegStr HKLM "Software\Classes\FeatherDocs.Documento\DefaultIcon" "" "$INSTDIR\featherdocs.ico"
  WriteRegStr HKLM "Software\Classes\FeatherDocs.Documento\shell\open\command" "" '"$INSTDIR\featherdocs.exe" "%1"'
  ; "Abrir com" para .docx, .tex e .md, sem tomar o lugar do programa padrão.
  WriteRegStr HKLM "Software\Classes\.docx\OpenWithProgids" "FeatherDocs.Documento" ""
  WriteRegStr HKLM "Software\Classes\.tex\OpenWithProgids" "FeatherDocs.Documento" ""
  WriteRegStr HKLM "Software\Classes\.md\OpenWithProgids" "FeatherDocs.Documento" ""

  WriteRegStr HKLM "Software\FeatherWorks\FeatherDocs" "Pasta" "$INSTDIR"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayName" "FeatherDocs"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKLM "${DESINSTALAR}" "Publisher" "FeatherWorks"
  ; Tamanho em "Programas e Recursos" (KB, do que foi instalado).
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${DESINSTALAR}" "EstimatedSize" "$0"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayIcon" "$INSTDIR\featherdocs.ico"
  WriteRegStr HKLM "${DESINSTALAR}" "URLInfoAbout" "https://github.com/feather-works/featherdocs"
  WriteRegStr HKLM "${DESINSTALAR}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegStr HKLM "${DESINSTALAR}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKLM "${DESINSTALAR}" "NoModify" 1
  WriteRegDWORD HKLM "${DESINSTALAR}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\FeatherDocs.lnk"
  Delete "$DESKTOP\FeatherDocs.lnk"
  Delete "$INSTDIR\featherdocs.exe"
  Delete "$INSTDIR\featherdocs.old"
  Delete "$INSTDIR\featherdocs.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\LICENCA-SINONIMOS.txt"
  Delete "$INSTDIR\LICENCA-DICIONARIO-INGLES.txt"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  ; A pasta da suíte só sai se ficou vazia (sem outro programa nem o Launcher).
  RMDir "$INSTDIR\.."
  DeleteRegKey HKLM "Software\Classes\FeatherDocs.Documento"
  DeleteRegValue HKLM "Software\Classes\.docx\OpenWithProgids" "FeatherDocs.Documento"
  DeleteRegValue HKLM "Software\Classes\.tex\OpenWithProgids" "FeatherDocs.Documento"
  DeleteRegValue HKLM "Software\Classes\.md\OpenWithProgids" "FeatherDocs.Documento"
  ReadRegStr $0 HKLM "Software\Classes\.fdoz" ""
  StrCmp $0 "FeatherDocs.Documento" 0 +2
    DeleteRegKey HKLM "Software\Classes\.fdoz"
  DeleteRegKey HKLM "Software\FeatherWorks\FeatherDocs"
  DeleteRegKey /ifempty HKLM "Software\FeatherWorks"
  DeleteRegKey HKLM "${DESINSTALAR}"
SectionEnd
