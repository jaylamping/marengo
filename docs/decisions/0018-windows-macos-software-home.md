# ADR 0018 Windows and macOS host checkouts

**Status:** Accepted
**Date:** 2026-09-29
**Supersedes:** [ADR 0016](0016-wsl-software-home.md)

## Context

The owner develops primarily on Windows and also uses a MacBook. A separate Ubuntu checkout and editor session made it harder to resume work and keep software and SolidWorks assets together. The September review recovered a WSL feature-branch checkout that was 94 commits behind current main, plus older Windows copies containing ignored local CAD.

## Decision

1. The Windows software home is `J:\code\marengo`. Local SolidWorks assemblies, parts, vendor assets, exports, meshes, calibration files, and Git history belong under `J:\code`. The primary CAD tree stays at `J:\code\marengo\cad`.
2. Windows software and CAD sessions open that same host checkout. macOS uses a host checkout of the same remote repository. Git carries tracked changes between machines.
3. Run the existing Linux build, full workspace tests, simulation, and virtual CAN checks through Docker Desktop and Compose. Ubuntu is not a separate development checkout or required editor session. The Pi and future Jetson remain Linux deployment targets.
4. Keep Cargo and npm build caches in Compose named volumes. Use pinned Rust and Node versions for native portable-crate and frontend work. Full native Windows Rust support is follow-up work because Chappe IPC currently uses Unix-only types.
5. Keep paths relative to the workspace. MCP launchers use Node; SolidWorks MCP remains a Windows-only sibling at `J:\code\solidworks-mcp` on this machine.
6. Preserve recovered Git changes and conflicting CAD versions before consolidating. Older copies are recovery sources, not active software homes. Do not overwrite the current five-joint master model with an older four-joint local export.

## Consequences

Windows and macOS share the same developer workflow while Linux-specific runtime behavior stays in the container and on the robot. Host bind mounts can cost more I/O than ext4; named volumes retain the expensive build caches. SolidWorks editing remains on Windows. Ignored CAD files need a separate backup or versioned CAD-vault policy because a normal Git clone on macOS does not restore them.

## References

- [Windows and macOS development](../windows-macos-development.md)
- [Docker Desktop host bind mounts](https://docs.docker.com/engine/storage/bind-mounts/)
- [Docker volumes](https://docs.docker.com/engine/storage/volumes/)
