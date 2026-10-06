; Block the ARM64 installer on non-ARM64 Windows (e.g. x64 PCs).
; The installer stub is an x86 binary, so under WOW64 the native architecture
; is in PROCESSOR_ARCHITEW6432; fall back to PROCESSOR_ARCHITECTURE otherwise.

!macro NSIS_HOOK_PREINSTALL
  !if "${ARCH}" == "arm64"
    ReadEnvStr $R8 PROCESSOR_ARCHITEW6432
    ${If} $R8 == ""
      ReadEnvStr $R8 PROCESSOR_ARCHITECTURE
    ${EndIf}
    ${If} $R8 != "ARM64"
      MessageBox MB_OK|MB_ICONSTOP "このインストーラーは ARM64 版です。お使いの PC (x64) では動作しません。x64 版のインストーラーをダウンロードしてください。$\r$\n$\r$\nThis is the ARM64 installer and cannot be installed on this PC (x64). Please download the x64 installer instead."
      SetErrorLevel 1
      Abort
    ${EndIf}
  !endif
!macroend
