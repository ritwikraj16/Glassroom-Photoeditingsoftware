# Glassroom

**Glassroom** is a free, open-source desktop photo app with a Classic-style local catalog workflow: import JPEGs, cull in a library grid, develop non-destructively, and export baked JPEGs. Originals on disk are never overwritten.

- **License:** [Apache-2.0](LICENSE)
- **Platform:** macOS-first; Linux/dev preview works today
- **Pipeline:** JPEG-only for MVP (no RAW decode yet)
- **Monetization:** none — no accounts, paywalls, or required telemetry

This project is **not** affiliated with Adobe. UI strings and branding are original (no Adobe / Lightroom trademarks).

## Try it on your Mac (recommended)

MVP ships as **source** you build once with Rust (no notarized `.dmg` yet).

1. Install Rust: https://rustup.rs  
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. Clone this repo (after it’s on GitHub):
   ```bash
   git clone https://github.com/<you>/<repo>.git
   cd <repo>
   ```
3. Build and run:
   ```bash
   cargo run --release
   ```
4. Click **Import Folder…** and pick JPEGs from **your** desktop/drive.

Demo JPEGs are in `demo-photos/` if you want a quick test:
```bash
./target/release/glassroom --import ./demo-photos
./target/release/glassroom
```

Catalog DB defaults to the app data dir (`Glassroom/catalog.db`). Override with `GLASSROOM_CATALOG=/path/to/catalog.db`.

### Linux (dev preview)

```bash
cargo run --release
```

## Features (MVP)

- Local SQLite catalog (path, star rating, pick/reject flag, collections)
- Import a folder of JPEGs
- Library: sidebar (folders / collections) + thumbnail grid + filmstrip
- Develop: exposure, contrast, highlights/shadows, white-balance temp/tint, saturation, clarity
- Non-destructive edit params stored in the catalog; export writes a new JPEG
- Dark module chrome with Library / Develop switcher

## Usage

1. **Import Folder…** — pick a directory of `.jpg` / `.jpeg` files.
2. Rate with `1`–`5`, clear with `0`; flag pick `P`, reject `X`, unflag `U`.
3. Switch to **Develop**, adjust sliders; preview updates; originals stay untouched.
4. **Export…** writes a baked JPEG (originals never overwritten).

## License

Copyright 2026 Glassroom contributors

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
