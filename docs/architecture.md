# Architecture

FFS is divided into three primary layers:

```text
┌───────────────────────────────────────────┐
│              Filesystem API               │
│          FFS / File / FsFormat            │
├───────────────────────────────────────────┤
│         Filesystem Format Layer           │
│                FormatV1                   │
│  Superblock / Inodes / Directories / ...  │
├───────────────────────────────────────────┤
│              Device Layer                 │
│ MemoryDevice / FileDevice / CachedDevice  │
└───────────────────────────────────────────┘
```

### Filesystem API

The public filesystem facade provides operations for:

* Formatting and mounting.
* Creating and deleting filesystem objects.
* Opening and closing files.
* Reading, writing, and seeking.
* Directory listing.
* Filesystem statistics.
* Flushing pending writes.
* Integrity checking.

The API is independent of the underlying filesystem format and storage device.

### Filesystem Formats

Filesystem formats implement the format-specific on-disk representation and operations required by the filesystem API.

The format is selected when mounting by inspecting metadata stored in the filesystem's superblock. This allows multiple filesystem-format versions to coexist behind the same API.

### Devices

The device layer abstracts block storage from the filesystem implementation. A filesystem operates on a `BlockDevice` rather than directly accessing a particular storage medium.

Current implementations include:

* `MemoryDevice`: in-memory block storage with filesystem-image import/export.
* `FileDevice`: block storage backed directly by a host file.
* `CachedDevice<D>`: an LRU write-back cache layered over another block device.



## Core Modules

| Module              | Responsibility                                           |
| ------------------- | -------------------------------------------------------- |
| `ffs.rs`            | Public filesystem facade and format selection            |
| `file.rs`           | Open-file handles and file types                         |
| `fs_error.rs`       | Filesystem errors and error codes                        |
| `fs_utils.rs`       | Shared constants and utilities                           |
| `device/`           | Block-device abstractions and implementations            |
| `formats/format.rs` | Format-independent filesystem interface and result types |
| `formats/v1/`       | Version 1 filesystem implementation                      |
