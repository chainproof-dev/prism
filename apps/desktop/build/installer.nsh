; NSIS installer hooks (docs/15 § 4): Explorer context-menu integration
; (parity WDS-CTX-04). Per-user writes HKCU; per-machine writes HKCR via
; the elevated page. Uninstall removes both.
!macro customInstall
  WriteRegStr SHCTX "Software\Classes\Directory\shell\AnalyzeWithPrism" "" "Analyze with Prism"
  WriteRegStr SHCTX "Software\Classes\Directory\shell\AnalyzeWithPrism" "Icon" "$INSTDIR\Prism.exe"
  WriteRegStr SHCTX "Software\Classes\Directory\shell\AnalyzeWithPrism\command" "" '"$INSTDIR\Prism.exe" "%V"'
!macroend

!macro customUnInstall
  DeleteRegKey SHCTX "Software\Classes\Directory\shell\AnalyzeWithPrism"
!macroend
