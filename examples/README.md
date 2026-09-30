# Examples

Examples of programmatic usage of the filesystem are located under `examples/`.

## Build and Run

Run the examples using the standard Rust command

    cargo run -example <example name>

FFS requires a Rust toolchain supporting **Rust Edition 2024**.

The currently available examples are
- `basic`

## Basic Example

1. Creates a 100-block `MemoryDevice`.
2. Wraps it in a 50-block `CachedDevice`.
3. Formats the device using Version 1.
4. Mounts the filesystem.
5. Creates files and directories.
6. Creates a hard link.
7. Performs file writes, reads, and seeks.
8. Lists directory contents.
9. Exports the filesystem image.
10. Runs the integrity checker.

> **Note:** Formatting initializes a new filesystem and overwrites the existing contents of the device.