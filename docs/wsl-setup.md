# WSL setup retired

The Marengo owner returned to Windows and macOS host development on 2026-09-29. Open `J:\code\marengo` on Windows; keep the software checkout and local CAD there.

Use [Windows and macOS development](windows-macos-development.md) and [ADR 0018](decisions/0018-windows-macos-software-home.md). Docker provides the Linux runtime checks. Ubuntu is no longer the software home or a required editor session.

The former setup procedure is preserved in Git history and [ADR 0016](decisions/0016-wsl-software-home.md). Legacy setup/deploy scripts are optional for explicitly requested WSL use.
