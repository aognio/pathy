# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows semantic
versioning.

## [0.1.0] - 2026-08-08

### Added

- Initial `pathy` command-line application for inspecting `PATH` entries.
- Unicode table renderer with aligned columns for index, status, path, type,
  executable counts, size, modified time, and duplicate status.
- Terminal capability detection for Unicode, ASCII fallback, color support, and
  dumb/non-TTY output.
- ASCII renderers with `compact`, `narrow`, and `minimal` formats.
- `--ascii-format <compact|narrow|minimal>` to explicitly select ASCII output.
- `--monochrome` to disable colored output.
- `--long` to preserve full path display when the terminal has enough room or
  when truncation should be disabled.
- `--folder-wrap` to wrap paths at folder boundaries while keeping one logical
  row per `PATH` entry.
- Directory, missing-entry, duplicate, executable-count, non-executable-count,
  size, and modified-time diagnostics.
- Tiny production build target via `make tiny`.
- GitHub Actions CI for formatting, tests, linting, and package validation.
- MIT license and project branding assets.

[0.1.0]: https://github.com/aognio/pathy/releases/tag/v0.1.0
