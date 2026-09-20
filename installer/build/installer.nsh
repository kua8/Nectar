; Custom NSIS behavior for the Nectar installer, wired in via Tauri's own
; `bundle.windows.nsis.installerHooks` config (tauri.conf.json) — NOT
; electron-builder, which turned out to be architecturally unable to wrap a
; native (non-Electron) executable correctly. Tauri's bundler builds the
; real installer around the actual compiled nectar.exe; this file only adds
; extra window styling on top of that, it doesn't replace any of Tauri's own
; install/uninstall logic.
;
; Goal: a dark, app-themed installer window instead of the stock light MUI2
; wizard — dark background matching the app/website (#0b0d13), and a
; borderless window with small square "traffic light" close/minimize
; controls in the top-left (mac position) instead of native title-bar
; buttons. The header/sidebar bitmaps and icons are set separately via
; tauri.conf.json's own headerImage/sidebarImage/installerIcon fields —
; nothing about those needs scripting here.
;
; Verification status: this exact file has been compiled clean (zero
; warnings) with the real makensis.exe (fetched by electron-builder's NSIS
; cache, version 3.0.4.1) against a minimal reproduction of Tauri's actual
; upstream installer.nsi structure — same include point (before any
; MUI_ICON/MUI_PAGE_* setup), same absence of pre-existing MUI_BGCOLOR/
; WS_CHILD definitions, confirmed by fetching Tauri's real template from
; github.com/tauri-apps/tauri and testing against it directly, not guessed.
; What is NOT verified: an actual `tauri build` run producing a real
; installer with the real nectar.exe (no cargo/rustc in this environment),
; or how the buttons actually look/behave on screen — expect to check sizes/
; positions once you can run a real build.

; WinMessages.nsh (already included by Tauri's template) does not define
; these — confirmed against the real compiler, "unknown variable/constant"
; — so they're spelled out as raw Win32 style bits instead of assuming a
; header provides them.
!define WS_CHILD_VAL   0x40000000
!define WS_VISIBLE_VAL 0x10000000
!define BS_FLAT_VAL    0x00008000

!define WS_SYSMENU_VAL  0x00080000
!define GWL_STYLE_VAL   -16
!define SW_MINIMIZE_VAL 6
!define SWP_FLAGS       0x0027 ; SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED

!define NECTAR_BG       "0B0D13" ; matches the app/website dark background
!define NECTAR_TEXT     "FFFFFF"
!define NECTAR_CLOSE    "FF5F57"
!define NECTAR_MINIMIZE "FEBC2E"
!define NECTAR_DECOR    "28C840"

!define MUI_BGCOLOR   ${NECTAR_BG}
!define MUI_TEXTCOLOR ${NECTAR_TEXT}

; MUI2's mechanism for wiring a callback into the real .onGUIInit — this
; needs to be set before `!insertmacro MUI_LANGUAGE` runs (near the end of
; Tauri's template), which it is, since installerHooks is !include'd very
; early (before any MUI_PAGE_* macro is inserted).
!define MUI_CUSTOMFUNCTION_GUIINIT NectarGuiInit

; Runs once, when the main installer window is first created — before any
; wizard page is shown. Controls created here as children of $HWNDPARENT (the
; persistent outer frame) stay visible across every page, since MUI2 swaps
; only the inner page content, not the outer window itself.
Function NectarGuiInit
  System::Call 'user32::GetWindowLongW(i $HWNDPARENT, i ${GWL_STYLE_VAL}) i .r0'
  IntOp $0 $0 & ~${WS_SYSMENU_VAL}
  System::Call 'user32::SetWindowLongW(i $HWNDPARENT, i ${GWL_STYLE_VAL}, i r0)'
  System::Call 'user32::SetWindowPos(i $HWNDPARENT, i 0, i 0, i 0, i 0, i 0, i ${SWP_FLAGS})'

  SetCtlColors $HWNDPARENT "${NECTAR_TEXT}" "${NECTAR_BG}"

  ; Three 14x14 squares in the mac position (top-left), 8px apart, 12px
  ; inset from the corner. Square rather than round — clipping a native
  ; button to a circle needs SetWindowRgn, one more layer of System.dll
  ; marshaling on top of everything else here; squares in the right
  ; position/colors get most of the way there without adding that risk.
  nsDialogs::CreateControl "BUTTON" ${WS_CHILD_VAL}|${WS_VISIBLE_VAL}|${BS_FLAT_VAL} 0 12 12 14 14 ""
  Pop $R0
  SetCtlColors $R0 "${NECTAR_CLOSE}" "${NECTAR_CLOSE}"
  GetFunctionAddress $R1 NectarCloseClick
  nsDialogs::OnClick $R0 $R1

  nsDialogs::CreateControl "BUTTON" ${WS_CHILD_VAL}|${WS_VISIBLE_VAL}|${BS_FLAT_VAL} 0 34 12 14 14 ""
  Pop $R2
  SetCtlColors $R2 "${NECTAR_MINIMIZE}" "${NECTAR_MINIMIZE}"
  GetFunctionAddress $R3 NectarMinimizeClick
  nsDialogs::OnClick $R2 $R3

  ; Decorative third dot only — a fixed-size wizard has no real
  ; restore/maximize state to wire this up to.
  nsDialogs::CreateControl "BUTTON" ${WS_CHILD_VAL}|${WS_VISIBLE_VAL}|${BS_FLAT_VAL} 0 56 12 14 14 ""
  Pop $R4
  SetCtlColors $R4 "${NECTAR_DECOR}" "${NECTAR_DECOR}"
  EnableWindow $R4 0
FunctionEnd

Function NectarCloseClick
  Quit
FunctionEnd

Function NectarMinimizeClick
  System::Call 'user32::ShowWindow(i $HWNDPARENT, i ${SW_MINIMIZE_VAL})'
FunctionEnd

; Tauri also supports NSIS_HOOK_PREINSTALL/POSTINSTALL/PREUNINSTALL/
; POSTUNINSTALL macros in this same file for install/uninstall-time actions,
; wrapped in `!ifmacrodef` so they're optional — intentionally not used here
; since this file is only about window styling, not install behavior.
;
; The uninstaller's window isn't styled to match — MUI2's equivalent hook
; for the uninstaller's GUI init isn't something this environment could
; confirm works the same way without another real compile test, and getting
; it wrong risks the uninstaller specifically, which matters more than a
; cosmetic mismatch there.
