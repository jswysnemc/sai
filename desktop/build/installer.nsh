; 目录右键：使用 Sai 打开此工作区
!macro customInstall
  WriteRegStr SHCTX "Software\Classes\Directory\shell\SaiDesktop" "" "使用 Sai 打开此工作区"
  WriteRegStr SHCTX "Software\Classes\Directory\shell\SaiDesktop" "Icon" "$INSTDIR\${APP_EXECUTABLE_FILENAME},0"
  WriteRegStr SHCTX "Software\Classes\Directory\shell\SaiDesktop\command" "" '"$INSTDIR\${APP_EXECUTABLE_FILENAME}" "%V"'
  WriteRegStr SHCTX "Software\Classes\Directory\Background\shell\SaiDesktop" "" "使用 Sai 打开此工作区"
  WriteRegStr SHCTX "Software\Classes\Directory\Background\shell\SaiDesktop" "Icon" "$INSTDIR\${APP_EXECUTABLE_FILENAME},0"
  WriteRegStr SHCTX "Software\Classes\Directory\Background\shell\SaiDesktop\command" "" '"$INSTDIR\${APP_EXECUTABLE_FILENAME}" "%V"'
!macroend

!macro customUnInstall
  DeleteRegKey SHCTX "Software\Classes\Directory\shell\SaiDesktop"
  DeleteRegKey SHCTX "Software\Classes\Directory\Background\shell\SaiDesktop"
!macroend
