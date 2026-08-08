# Pathy

A modern, visual command-line tool for exploring and editing the `PATH` environment variable.

## Goals

- Display PATH entries in a readable table.
- Preserve and display PATH entry ordering.
- Quickly indicate whether each directory exists.
- Count executable binaries in each directory.
- Report directory size.
- Provide an interactive `--edit` mode for rearranging PATH entries.
- Generate the resulting PATH export for the user's shell.
- Eventually support applying PATH changes to the appropriate shell configuration.

## Development

Build:

```text
cargo build