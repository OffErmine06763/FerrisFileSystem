# Programs

Programs to format/mount/query a device.

## Build and Run

Run the individual programs using

    cargo build --bin <program_name>
    cargo run   --bin <program_name>

The currently available programs are
- `fuse_mount`
- `mkfs`

## FUSE Mount

TODO: pass storage and mount point as command line arguments (./ffs-fuse-mount /storage/path /mount/path)

On LINUX only, with fuse feature enabled.

    cargo run --bin fuse_mount --features fuse

## mkfs

Formats the given device.

    ./ffs-mkfs storage/path

TODO