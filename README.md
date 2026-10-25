# Glassroom

**Glassroom** is a free, open-source desktop photo app with a Classic-style local catalog workflow: import JPEGs, cull in a library grid, develop non-destructively, and export baked JPEGs. Originals on disk are never overwritten.

- **License:** [Apache-2.0](LICENSE)
- **Platform:** macOS-first; Linux/dev preview works today
- **Pipeline:** JPEG-only for MVP (no RAW decode yet)
- **Monetization:** none — no accounts, paywalls, or required telemetry

This project is **not** affiliated with Adobe. UI strings and branding are original (no Adobe / Lightroom trademarks).

## Features (MVP)

- Local SQLite catalog (path, star rating, pick/reject flag, collections)
- Import a folder of JPEGs
- Library: sidebar (folders / collections) + thumbnail grid + filmstrip
- Develop: exposure, contrast, highlights/shadows, white-balance temp/tint, saturation, clarity
- Non-destructive edit params stored in the catalog; export writes a new JPEG
- Dark module chrome with Library / Develop switcher

## Build

### Linux (dev preview)

```bash
cd glassroom
cargo run --release
```

### macOS

```bash
# Requires Rust toolchain: https://rustup.rs
cd glassroom
cargo build --release
./target/release/glassroom
```

Catalog data defaults to the platform app-data directory (`Glassroom/catalog.db`). Demo mode can also use a path next to the binary when `GLASSROOM_CATALOG` is set.

## Usage

1. **File → Import Folder…** (or the Import button) — pick a directory of `.jpg` / `.jpeg` files.
2. Rate with `1`–`5`, clear with `0`; flag pick `P`, reject `X`, unflag `U`.
3. Switch to **Develop**, adjust sliders; preview updates; originals stay untouched.
4. **Export…** writes a baked JPEG beside the original (or a chosen folder).

## License

Copyright 2026 Glassroom contributors

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
