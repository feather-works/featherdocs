; Instalador do FeatherDocs para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/featherdocs.exe -DSAIDA=caminho/instalador.exe pacote/featherdocs.nsi
; Instala por usuário (sem pedir administrador), cria atalhos, associa o .fdoz
; e registra o desinstalador em "Aplicativos instalados".

Unicode true
!include "MUI2.nsh"

!ifndef VERSAO
  !define VERSAO "0.0.0"
!endif

Name "FeatherDocs ${VERSAO}"
OutFile "${SAIDA}"
InstallDir "$LOCALAPPDATA\Programs\FeatherDocs"
InstallDirRegKey HKCU "Software\FeatherWorks\FeatherDocs" "Pasta"
RequestExecutionLevel user
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

!define DESINSTALAR "Software\Microsoft\Windows\CurrentVersion\Uninstall\FeatherDocs"

Section "FeatherDocs"
  SetOutPath "$INSTDIR"
  File "/oname=featherdocs.exe" "${EXE}"
  File "icones\featherdocs.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherDocs.lnk" "$INSTDIR\featherdocs.exe" "" "$INSTDIR\featherdocs.ico"
  CreateShortCut "$DESKTOP\FeatherDocs.lnk" "$INSTDIR\featherdocs.exe" "" "$INSTDIR\featherdocs.ico"

  ; Documento do FeatherDocs (.fdoz): ícone e abrir com dois cliques.
  WriteRegStr HKCU "Software\Classes\.fdoz" "" "FeatherDocs.Documento"
  WriteRegStr HKCU "Software\Classes\FeatherDocs.Documento" "" "Documento do FeatherDocs"
  WriteRegStr HKCU "Software\Classes\FeatherDocs.Documento\DefaultIcon" "" "$INSTDIR\featherdocs.ico"
  WriteRegStr HKCU "Software\Classes\FeatherDocs.Documento\shell\open\command" "" '"$INSTDIR\featherdocs.exe" "%1"'
  ; "Abrir com" para .docx, .tex e .md, sem tomar o lugar do programa padrão.
  WriteRegStr HKCU "Software\Classes\.docx\OpenWithProgids" "FeatherDocs.Documento" ""
  WriteRegStr HKCU "Software\Classes\.tex\OpenWithProgids" "FeatherDocs.Documento" ""
  WriteRegStr HKCU "Software\Classes\.md\OpenWithProgids" "FeatherDocs.Documento" ""

  WriteRegStr HKCU "Software\FeatherWorks\FeatherDocs" "Pasta" "$INSTDIR"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayName" "FeatherDocs"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKCU "${DESINSTALAR}" "Publisher" "FeatherWorks"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayIcon" "$INSTDIR\featherdocs.ico"
  WriteRegStr HKCU "${DESINSTALAR}" "URLInfoAbout" "https://github.com/feather-works/featherdocs"
  WriteRegStr HKCU "${DESINSTALAR}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegDWORD HKCU "${DESINSTALAR}" "NoModify" 1
  WriteRegDWORD HKCU "${DESINSTALAR}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\FeatherDocs.lnk"
  Delete "$DESKTOP\FeatherDocs.lnk"
  Delete "$INSTDIR\featherdocs.exe"
  Delete "$INSTDIR\featherdocs.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Classes\FeatherDocs.Documento"
  DeleteRegValue HKCU "Software\Classes\.docx\OpenWithProgids" "FeatherDocs.Documento"
  DeleteRegValue HKCU "Software\Classes\.tex\OpenWithProgids" "FeatherDocs.Documento"
  DeleteRegValue HKCU "Software\Classes\.md\OpenWithProgids" "FeatherDocs.Documento"
  ReadRegStr $0 HKCU "Software\Classes\.fdoz" ""
  StrCmp $0 "FeatherDocs.Documento" 0 +2
    DeleteRegKey HKCU "Software\Classes\.fdoz"
  DeleteRegKey HKCU "Software\FeatherWorks\FeatherDocs"
  DeleteRegKey HKCU "${DESINSTALAR}"
SectionEnd
