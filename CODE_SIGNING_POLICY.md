# Code signing policy

Free code signing provided by SignPath.io, certificate by SignPath Foundation.

## Scope

This repository contains the source code, build scripts, and GitHub Actions
workflows for the Deni AI Desktop application.

Only artifacts built from this repository are eligible for project code
signing. Third-party or upstream binaries may be included in release packages
when permitted by their licenses, but they are not re-signed as if they were
produced by this project.

## Team roles

- Committers and reviewers: [@raicdev](https://github.com/raicdev)
- Approvers: [@raicdev](https://github.com/raicdev)

The current maintainer is responsible for source control, release preparation,
and manual approval of signing requests. This document must be updated whenever
the project team or release authority changes.

## Security requirements

- All maintainers participating in code signing must use multi-factor
  authentication for GitHub and SignPath access.
- Stable Windows signing requests are submitted from GitHub-hosted runners.
- Each signing request requires manual approval before signed artifacts are
  published.
- Release tags must match the application version declared in `package.json`,
  `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

## Release policy

- Only this project's own Windows release artifacts are submitted to SignPath
  for Authenticode signing.
- Stable releases are created from version tags matching `v*`.
- The project does not publish hidden system modifications, bundled malware, or
  security-circumvention features as part of signed releases.
- Unsupported destinations are opened in the system browser instead of being
  loaded in-app.

## Privacy policy

The desktop shell itself does not add its own telemetry pipeline. It loads the
hosted Deni AI service and related first-party endpoints needed for sign-in,
downloads, and updates.

- Desktop shell privacy summary: [PRIVACY.md](./PRIVACY.md)
- Hosted Deni AI privacy policy:
  `https://deniai.app/legal/privacy-policy`

Unless specifically initiated by the user, the desktop shell does not transfer
information to arbitrary third-party network services outside the hosted Deni AI
service and system-integrated browser behavior.
