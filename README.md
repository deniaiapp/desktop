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

## Security

If you discover a vulnerability, do not open a public issue. Follow the process
in [SECURITY.md](./SECURITY.md).

## Community

- [Contributing](./CONTRIBUTING.md)
- [Code of Conduct](./CODE_OF_CONDUCT.md)
- [License](./LICENSE.md)
