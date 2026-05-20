# HD Analyzer

A Tauri desktop disk usage analyzer built with Rust, React, and shadcn/ui.

![HD Analyzer logo](src-web/public/logo.svg)

## Overview

HD Analyzer gives you a desktop view into storage usage. It uses a Rust scanner backend through Tauri commands and a React frontend for drive selection, scan progress, hierarchical browsing, category breakdowns, and permission/error visibility.

## Key Features

- **Parallel scanning**: Uses `rayon` to traverse filesystems using available CPU cores.
- **Desktop UI**: Tauri v2 shell with a React + shadcn/ui frontend.
- **Scan progress**: Tracks active scans and updates the explorer as results become available.
- **Permission handling**: Checks macOS Full Disk Access state and exposes the app permission request from the desktop UI.
- **Access debugging**: Shows unreadable paths and scan errors from the Rust backend.
- **Smart navigation**: Respects filesystem boundaries to avoid unexpected mounted-drive traversal.
- **Usage distribution**: Breaks usage down by file categories such as apps, media, code, and archives.

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- [Node.js](https://nodejs.org/) and npm for the Tauri desktop frontend
- Tauri v2 system dependencies for your platform

### Build from source
```bash
git clone https://github.com/vanrez-nez/hd-analyzer.git
cd hd-analyzer
npm --prefix src-web install
npm --prefix src-web run tauri:build
```

## Usage

Run the Tauri desktop app from the repository root:
```bash
npm --prefix src-web install
npm --prefix src-web run tauri:dev
```

Validate the workspace:
```bash
cargo fmt --check
cargo test
npm --prefix src-web run typecheck
npm --prefix src-web run build
```

## Built With
- [Rust](https://www.rust-lang.org/)
- [Tauri](https://tauri.app/)
- [React](https://react.dev/)
- [shadcn/ui](https://ui.shadcn.com/)
- [Rayon](https://github.com/rayon-rs/rayon)
