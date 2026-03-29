# Contributing to Assembly Simulator

Thank you for your interest in improving this project. This document explains how to report issues, propose changes, and submit code.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating, you agree to uphold it.

## Ways to contribute

- **Bug reports** — reproducible steps, expected vs actual behavior, OS version, and app version.
- **Feature ideas** — open an issue describing the problem and proposed solution.
- **Documentation** — README fixes, examples, or clarifications.
- **Code** — bugfixes, tests, new architecture features, or UI improvements (see below).

## Before you start coding

1. **Search existing issues** to avoid duplicates.
2. **Discuss large changes** in an issue first (new ISA, major UI refactor).
3. **Keep changes focused** — one logical change per pull request when possible.

## Development setup

Prerequisites:

- **Node.js** 18+ and npm  
- **Rust** (stable), via [rustup](https://rustup.rs/)  
- **Platform**: Tauri targets desktop; macOS is the primary tested platform.

```bash
git clone https://github.com/AMANKANOJIYA/simulator.git
cd simulator
npm install
npm run tauri:dev    # desktop app with hot reload
```

Use your fork’s URL if you cloned a fork (e.g. `https://github.com/<your-username>/simulator.git`) instead of the upstream URL above.

Other useful commands:

| Command | Purpose |
|--------|---------|
| `npm run build` | Typecheck + Vite production build |
| `npm run lint` | ESLint on the frontend |
| `cd src-tauri && cargo check` | Check Rust without full app |
| `cd src-tauri && cargo fmt` | Format Rust code |
| `cd src-tauri && cargo clippy` | Rust lints (if configured) |

## Project layout (short)

| Area | Path | Notes |
|------|------|--------|
| Frontend | `src/` | React + TypeScript, Zustand, Monaco |
| Backend | `src-tauri/src/` | Rust simulator, Tauri commands, architecture plugins |
| Plugins | `src-tauri/src/plugin/` | One module per ISA (`rv32i`, `lc3`, `mips`, `i8085`, `i6502`, `i8086`, …) |
| Samples | `src/samples.ts` | Example programs per architecture |

## Style guidelines

### TypeScript / React

- Prefer **functional components** and hooks.
- Run **`npm run lint`** and fix issues before submitting.
- Match existing formatting (no unrelated reformatting in the same PR).
- Use meaningful names; add short comments where behavior is non-obvious.

### Rust

- Run **`cargo fmt`** on changed files.
- Follow the existing **`ArchitecturePlugin`** pattern when touching ISAs.
- Prefer small, testable functions; avoid large copy-paste across plugins without abstraction.

### UI / UX

- Respect **theme tokens** (`themes.css`, `layout-shell.css`) and accessibility (contrast, focus, reduced motion).
- Test in **dark and light** themes when changing chrome.

## Pull request process

1. Fork the repository and create a branch from `main` (or the default branch), e.g. `fix/memory-jump` or `feat/lc3-directive`.
2. Make your changes with clear commits (optional but helpful).
3. Ensure **`npm run build`** and **`npm run lint`** pass; fix any new issues.
4. For Rust changes, ensure **`cargo check`** (or `cargo test` if tests exist) passes.
5. Open a PR with:
   - A clear **title** and **description** (what / why).
   - Links to **related issues** (`Fixes #123` when applicable).
   - **Screenshots** or short notes for UI changes.

Maintainers will review as time allows. Feedback may request small changes before merge.

## Security

Please do **not** open public issues for security vulnerabilities. See [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions will be licensed under the same license as the project ([MIT](LICENSE)).
