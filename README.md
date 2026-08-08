# Pathy

A modern, visual command-line tool for exploring the `PATH` environment variable.

## Install

Install the current GitHub version:

```text
cargo install --git https://github.com/aognio/pathy --branch wip
```

Build from a local checkout:

```text
cargo install --path .
```

Build the smallest production binary:

```text
make tiny
```

## Usage

```text
pathy
pathy --long
pathy --folder-wrap
pathy --ascii-format compact
pathy --ascii-format narrow
pathy --ascii-format minimal
pathy --monochrome
pathy --tui
```

## Goals

- Display PATH entries in a readable table.
- Preserve and display PATH entry ordering.
- Quickly indicate whether each directory exists.
- Count executable and non-executable files in each directory.
- Report directory size and modification age.
- Detect terminal capabilities and degrade from Unicode to ASCII output.
- Eventually provide an interactive doctor/editor for fixing PATH ordering.

## Development

Build:

```text
cargo build
```

Test and lint:

```text
cargo test
cargo clippy --all-targets -- -D warnings
```

## License

MIT License - Copyright (c) 2026 Antonio Ognio

Made with ❤️ from 🇵🇪. El Perú es clave 🔑.
