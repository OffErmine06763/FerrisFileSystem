# FerrisFileSystem

**FerrisFileSystem (FFS)** is a custom filesystem implemented in Rust. The project provides a complete filesystem implementation centered on its on-disk representation, storage management, and filesystem operations.

The filesystem is exposed through Rust APIs and currently operates independently of an operating system. Integration with an OS through **FUSE** is planned for a future stage.

The implementation is organized around a modular architecture that separates the filesystem interface, storage devices, and filesystem-format implementations. This allows additional filesystem formats and storage backends to be introduced without coupling them to the public filesystem interface.

## Purpose and Scope

FerrisFileSystem is an educational project intended to explore the design and implementation of filesystem concepts, including on-disk data structures, block allocation, inode management, directory structures, filesystem operations, storage abstractions, caching, and integrity checking.

The project is not intended for production use and should not be used as a replacement for established, production-grade filesystems. It is a relatively small implementation and does not currently provide many of the safety, reliability, performance, and recovery mechanisms expected from production filesystem implementations.

In particular, the current implementation has limitations regarding crash consistency, recovery, concurrency, security, durability guarantees, and operating-system integration. These limitations are expected to evolve as the project develops.

The primary objective of FerrisFileSystem is therefore learning, experimentation, and exploration of filesystem design, rather than providing a production-ready storage solution.

## Features

* Filesystem formatting and mounting.
* Regular files, directories, symbolic links, and hard links.
* File creation, deletion, opening, reading, writing, and seeking.
* Directory creation, traversal, and listing.
* Symbolic and hard link management.
* Free-space accounting and allocation.
* Block-level caching with LRU eviction and write-back.
* Filesystem image import and export.
* Filesystem integrity checking.
* Multiple storage-device backends through a common block-device abstraction.
* Versioned filesystem-format architecture.
* OS integration on Linux using FUSE.

## Additional Documentation, Programs, and Usage Examples

Additional documentation about the project is located under
- `docs/`: generic codebase documentation
- [`examples/`](examples/examples.md): examples of the API usage
- [`bin/`](src/bin/bin.md): for executables documentation

## Repository Structure

```text
src/
├── main.rs                   # example usage of the filesystem API
├── ffs.rs                    # filesystem API
├── file.rs                   # file handler
├── fs_error.rs
├── fs_utils.rs
│
├── device/                   # handlers of the physical location of the storage
│   ├── block_device.rs       # location-independent API
│   ├── memory_device.rs      # in-memory (RAM) storage
│   ├── file_device.rs        # storage in an existing file
│   └── cached_device.rs      # LRU write-back block cache
│
├── formats/
│   ├── format.rs             # format independent internal filesystem API
│   │
│   └── v1/                   # on-disk format version 1
│       ├── format.rs
│       └── ...
│   
└── bin/                      # standalone executables

examples/                     # API usage examples
```

## Dependencies

FFS requires a Rust toolchain supporting **Rust Edition 2024**.

### FUSE [optional, Linux only]

Optionally, on Linux it's possible to enable FUSE usage by specifying

    --features fuse

when building/running the examples and bins.  
This requires fuse installed.
