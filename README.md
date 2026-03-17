# LilyPond Studio

Desktop LilyPond scratchpad built with Rust, `gpui`, and `gpui-component`.

## Features

- GPUI desktop UI with a multiline LilyPond editor
- LilyPond CLI integration that renders fresh SVG pages on demand
- In-app preview for the generated score pages
- Embedded tutorial section with LilyPond syntax lessons and loadable examples

## Requirements

- Rust toolchain
- LilyPond installed and available on `PATH`

## Run

```bash
cargo run
```

The app starts with a sample score, renders it once on launch, and lets you re-render after edits with the `Render SVG` button.
