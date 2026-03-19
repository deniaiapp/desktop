# Contributing

## Overview

Thanks for contributing to Deni AI Desktop.

This repository contains a Tauri-based desktop shell for the hosted Deni AI web
app at `https://deniai.app/chat`.

## Before You Start

- Review [README.md](./README.md) for project setup and scripts
- Review [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) before participating
- Do not use public issues for security reports; follow [SECURITY.md](./SECURITY.md)

## Development Setup

1. Install Bun 1.3 or later.
2. Install the Rust toolchain.
3. Install the Windows tooling required by Tauri.
4. Ensure Microsoft WebView2 Runtime is available.
5. Install dependencies with `bun install`.
6. Start the app with `bun run dev`.

## Project Notes

- The app loads the hosted Deni AI site instead of bundling a local frontend
- Allowed in-app navigation is intentionally restricted
- Unsupported destinations should open in the system browser
- Stable and canary builds use different bundle identifiers

## Submitting Changes

- Keep changes focused and easy to review
- Update documentation when behavior or workflows change
- Prefer small pull requests over large unrelated batches of changes
- Include a clear summary of what changed and why
- Add screenshots when UI or packaging behavior changes in a visible way

## Commit Messages

Use Conventional Commits for new commits.

- `feat:` for user-facing features
- `fix:` for bug fixes
- `docs:` for documentation-only changes
- `chore:` for maintenance work

Examples:

- `feat: add canary bundle config`
- `fix: restrict external navigation`
- `docs: add security policy`

## Testing

Before submitting a pull request, run the checks that match your change.

- For local development: `bun run dev`
- For build verification: `bun run build:dev`
- For release-oriented verification: `bun run build:stable`
- For canary packaging verification: `bun run build:canary`

If you cannot run a relevant check, note that in the pull request.

## Pull Requests

- Describe the user-visible impact
- Mention platform-specific behavior when relevant
- Link related issues if they exist
- Keep commits and file changes scoped to the problem being solved

## Security

If your contribution touches security-sensitive behavior such as navigation
rules, authentication flow handling, or external URL opening, call that out in
the pull request description.
