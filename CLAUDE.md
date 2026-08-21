# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Read `AGENTS.md` first. It is the canonical guidance and this file is only a pointer.** Commands, architecture, single-source helpers, guardrails, and styling rules all live there, so Claude, Codex, and other agents share one source of truth instead of drifting instructions.

Directory-local guides layer on top of the root file. Read the one for the app you are editing before changing it:

| Editing                                | Read                                                       |
| -------------------------------------- | ---------------------------------------------------------- |
| Desktop frontend (`apps/desktop/src/`) | `apps/desktop/AGENTS.md` + `src/styles/COMPONENT_GUIDE.md` |
| Desktop Rust backend                   | `apps/desktop/src-tauri/AGENTS.md`                         |
| MCP server                             | `apps/mcp-server/AGENTS.md`                                |

Every one of those directories also has a `CLAUDE.md` pointing at its `AGENTS.md`. When guidance changes, edit the `AGENTS.md`, never the pointer.
