# Deni AI Desktop

Desktop wrapper for `https://deniai.app/chat` built with Tauri.

## Overview

This repository packages the Deni AI web app as a native desktop application.
The app opens `https://deniai.app/chat` in a Tauri webview and keeps supported
in-app navigation inside the desktop shell.

## Features

- Loads the Deni AI chat app directly from `https://deniai.app/chat`
- Keeps Deni AI chat and sign-in routes inside the desktop window
- Allows Google sign-in flow required by the hosted app
- Opens unsupported external URLs in the system browser
- Restores the main window size and position between launches
- Minimizes to the system tray instead of exiting on close
- Adds native desktop menu actions for reload, retry, history, zoom, and browser handoff
- Surfaces native notifications for downloads, background updates, and tray behavior
- Tracks the latest download and opens the Downloads folder from the app menu or tray
- Provides separate stable and canary desktop bundle identifiers

## Prerequisites

- Bun 1.3 or later
- Rust toolchain (`rustup`, `cargo`, `rustc`)
- Windows build tooling for Tauri
- Microsoft WebView2 Runtime on Windows

## Development

```bash
bun install
bun run dev
```

Available scripts:

- `bun run start`
- `bun run dev`
- `bun run dev:watch`
- `bun run build`
- `bun run build:dev`
- `bun run build:stable`
- `bun run build:canary`

## Build Targets

- Stable bundle identifier: `app.deniai.desktop`
- Canary bundle identifier: `app.deniai.desktop.canary`

The canary build can be installed alongside the stable build.

## Navigation Rules

The desktop shell keeps these flows inside the app:

- `https://deniai.app/chat`
- `https://deniai.app/chat/*`
- `https://deniai.app/auth/sign-in`
- `https://deniai.app/auth/sign-in/*`
- Google authentication callback handling used by the hosted app

Any other destination is opened in the system browser.

## Desktop Behavior

- Closing the main window hides it to the system tray
- Left-clicking the tray icon restores the window
- `Help > Check for Updates` is wired in, but it needs a release feed and signing key configuration before it can deliver production updates

## Security

If you discover a vulnerability, do not open a public issue. Follow the process
in [SECURITY.md](./SECURITY.md).

## Code Signing Policy

The project code signing policy is published in
[CODE_SIGNING_POLICY.md](./CODE_SIGNING_POLICY.md).

Stable Windows release artifacts are intended to be code signed through
SignPath.io using a SignPath Foundation certificate after manual approval.

## Community

- [Contributing](./CONTRIBUTING.md)
- [Code of Conduct](./CODE_OF_CONDUCT.md)
- [License](./LICENSE.md)
