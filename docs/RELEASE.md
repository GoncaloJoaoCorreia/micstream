# Release & Versioning Guide

This document describes the automated release pipeline, versioning conventions, and release staging procedures for **MicStream**.

---

## Table of Contents

1. [Versioning Strategy](#versioning-strategy)
2. [Changelog Standards](#changelog-standards)
3. [Automated CI/CD Release Pipeline](#automated-cicd-release-pipeline)
   - [Pipeline Architecture](#pipeline-architecture)
   - [Job 1: Prepare Release & Notes (`prepare-release`)](#job-1-prepare-release--notes-prepare-release)
   - [Job 2: Cross-Platform Matrix Build (`build-binaries`)](#job-2-cross-platform-matrix-build-build-binaries)
   - [Job 3: Atomic GitHub Release Publication (`publish-release`)](#job-3-atomic-github-release-publication-publish-release)
4. [Step-by-Step Release Playbook](#step-by-step-release-playbook)
5. [Platform Specifics & Code Signing](#platform-specifics--code-signing)
6. [Local Build Verification](#local-build-verification)

---

## Versioning Strategy

MicStream follows [Semantic Versioning 2.0.0](https://semver.org/) (`vMAJOR.MINOR.PATCH`):

- **MAJOR (`X.0.0`)**: Incompatible protocol changes, breaking IPC interfaces, or major architectural overhauls.
- **MINOR (`0.X.0`)**: New features, new codec support, platform additions, or backwards-compatible protocol extensions.
- **PATCH (`0.0.X`)**: Bug fixes, performance optimizations, dependency updates, and documentation improvements.

Release tags in Git use the `v` prefix (e.g. `v0.1.0`, `v0.1.1`).

---

## Changelog Standards

MicStream adheres strictly to the [Keep a Changelog (v1.1.0)](https://keepachangelog.com/en/1.1.0/) format in `CHANGELOG.md`.

### Format Requirements
- Each released version must have a dedicated level-2 heading formatted exactly as:
  ```markdown
  ## [X.Y.Z] - YYYY-MM-DD
  ```
- Subsections should categorize changes using standard verbs:
  - `### Added` for new features.
  - `### Changed` for changes in existing functionality.
  - `### Deprecated` for soon-to-be removed features.
  - `### Removed` for now removed features.
  - `### Fixed` for any bug fixes.
  - `### Security` in case of vulnerabilities.

---

## Automated CI/CD Release Pipeline

Native release binaries are compiled, tested, and published automatically via GitHub Actions whenever changes are pushed or merged to the `main` branch.

### Pipeline Architecture

```
+─────────────────────────────────────────────────────────────+
│                       Push to main                          │
+──────────────────────────────┬──────────────────────────────+
                               │
                               ▼
+─────────────────────────────────────────────────────────────+
│           prepare-release (ubuntu-latest, ~5 sec)           │
│   • Calculate next patch version (e.g. v0.1.0 -> v0.1.1)    │
│   • Validate CHANGELOG.md contains matching ## [X.Y.Z]      │
│   • Extract release notes to artifact                       │
+───────────────────────┬─────────────────────────────┬───────+
                        │                             │
                        ▼                             ▼
+──────────────────────────────+ +────────────────────────────+
│  macos-latest (matrix)       │ │  windows-latest (matrix)   │
│  • npm ci & cargo test       │ │  • npm ci & cargo test     │
│  • npm run tauri build       │ │  • npm run tauri build     │
│  • Upload .dmg artifact      │ │  • Upload NSIS .exe setup  │
+───────────────────────┬──────+ +─────────────────────┬──────+
                        │                             │
                        └──────────────┬──────────────┘
                                       │ All builds succeed
                                       ▼
+─────────────────────────────────────────────────────────────+
│           publish-release (ubuntu-latest)                   │
│   • Download macOS and Windows artifacts                    │
│   • Create annotated Git tag vX.Y.Z                         │
│   • Publish GitHub Release with notes and native installers │
+─────────────────────────────────────────────────────────────+
```

### Job 1: Prepare Release & Notes (`prepare-release`)
- **Runner**: `ubuntu-latest` (fast startup, ~5 seconds).
- **Version Calculation**:
  - Checks existing Git tags matching `v*.*.*`.
  - Increments the patch version (`v0.1.0` $\to$ `0.1.1`). If no tags exist, it uses the version declared in `package.json`.
- **Changelog Validation**:
  - Executes `.github/scripts/extract-changelog.sh` to extract the corresponding section from `CHANGELOG.md`.
  - **Fail-Fast Safety Gate**: If `CHANGELOG.md` is missing the heading `## [X.Y.Z]`, the workflow fails immediately before matrix build runners are allocated, saving CI minutes and preventing empty release notes.
- **Artifact**: Uploads `release-notes.md`.

### Job 2: Cross-Platform Matrix Build (`build-binaries`)
Runs in parallel across platforms:
- **macOS (`macos-latest`)**:
  - Installs Node.js 20 and stable Rust toolchain.
  - Updates version in `package.json` and `src-tauri/tauri.conf.json`.
  - Executes `cargo test --workspace` to ensure all tests pass.
  - Executes `npm run tauri build` to generate the `.dmg` bundle.
  - Uploads `.dmg` artifact (`target/release/bundle/dmg/*.dmg`).
- **Windows (`windows-latest`)**:
  - Installs Node.js 20 and stable Rust toolchain.
  - Updates version in `package.json` and `src-tauri/tauri.conf.json`.
  - Executes `cargo test --workspace`.
  - Executes `npm run tauri build` to generate the NSIS `.exe` installer.
  - Uploads NSIS setup installer artifact (`target/release/bundle/nsis/*-setup.exe`).

### Job 3: Atomic GitHub Release Publication (`publish-release`)
- Waits for all matrix build jobs to succeed (`fail-fast: false` ensures all errors are visible).
- Downloads release notes and all binary artifacts (`binary-macos`, `binary-windows`).
- Creates the official Git tag `vX.Y.Z` and publishes the GitHub Release with attached installers and formatted changelog notes.

---

## Step-by-Step Release Playbook

To release a new version of MicStream:

### 1. Update `CHANGELOG.md`
Move items from the `## [Unreleased]` section into a new version heading matching the target release version and current date:

```markdown
## [Unreleased]

## [0.1.1] - 2026-09-29

### Added
- Auto-discovery retry mechanism on network interface switch.

### Fixed
- Fixed audio buffer underrun when switching playback devices on Windows.
```

### 2. Commit and Push to `main`
Commit the changelog update and push directly to `main` (or merge an approved Pull Request):

```bash
git add CHANGELOG.md
git commit -m "chore(release): prepare v0.1.1"
git push origin main
```

### 3. Monitor GitHub Actions
1. Navigate to the **Actions** tab on GitHub.
2. Watch the **Release** workflow run:
   - `prepare-release` validates the changelog and computes `v0.1.1`.
   - `build-binaries` builds both macOS and Windows installers in parallel.
   - `publish-release` creates the release and attaches `.dmg` and `.exe` installers.

---

## Platform Specifics & Code Signing

### macOS Gatekeeper
Unsigned `.dmg` releases will trigger Apple Gatekeeper on first launch.
- **Workaround for Users**: Right-click (or Control-click) `MicStream.app` in Finder, select **Open**, and click **Open** in the dialog.

### Windows SmartScreen
Unsigned Windows `.exe` installers may trigger Windows Defender SmartScreen.
- **Workaround for Users**: Click **More info** $\to$ **Run anyway**.

---

## Local Build Verification

To verify builds locally prior to cutting a release:

```bash
# 1. Check Rust workspace compilation and unit tests
cargo test --workspace

# 2. Check TypeScript types and frontend build
npm run build

# 3. Build native desktop installer locally
npm run tauri build
```
