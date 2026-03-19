# Repository Guidelines

## Project Structure & Module Organization

This repository is a small Tauri desktop shell for `https://deniai.app/chat`.

- `package.json`: Bun-based developer commands
- `src-tauri/src/`: Rust application entry points (`main.rs`, `lib.rs`)
- `src-tauri/tauri.conf.json`: stable app config
- `src-tauri/tauri.canary.conf.json`: canary app config
- `src-tauri/icons/`: bundled app icons
- `build/`: generated build artifacts

There is no separate frontend app in this repo; the desktop shell loads the hosted site directly.

## Build, Test, and Development Commands

- `bun install`: install JS-side dependencies
- `bun run dev`: start the Tauri app in development mode
- `bun run build:dev`: produce a debug desktop build
- `bun run build:stable`: produce the stable release build
- `bun run build:canary`: build the canary variant with its own bundle identifier

Run commands from the repository root.

## Coding Style & Naming Conventions

Rust code in `src-tauri/src/` follows the existing style:

- use 2-space indentation
- prefer small helper functions over large inline closures
- keep constants uppercase, e.g. `APP_ORIGIN`
- use clear labels for Tauri resources, e.g. `MAIN_WINDOW_LABEL`

Use standard Rust formatting before opening a PR: `cargo fmt --manifest-path src-tauri/Cargo.toml`.

## Testing Guidelines

There is no automated test suite yet. For now, contributors should verify:

- the app launches with `bun run dev`
- allowed in-app routes still stay inside the window
- external links still open in the system browser
- stable and canary builds both package successfully when touched

Document manual verification in the PR description.

## Commit & Pull Request Guidelines

Use Conventional Commits for new history:

- `feat:` for user-facing features
- `fix:` for bug fixes
- `docs:` for documentation-only changes
- `chore:` for maintenance work

Examples: `feat: add canary bundle config`, `fix: restrict external navigation`.

Pull requests should include:

- a clear summary of the change
- linked issues, if any
- manual test notes
- screenshots when window behavior or packaging output changes

## Security & Configuration Tips

Do not expose new in-app navigation paths without reviewing authentication and external URL handling in `src-tauri/src/lib.rs`. Report vulnerabilities privately per `SECURITY.md`.
