# Roadmap

## Current Status

FFS currently provides a functional filesystem implementation with a versioned on-disk format, multiple storage backends, caching, filesystem links, file and directory operations, and integrity checking.

Planned future work includes operating-system integration through **FUSE** and additional filesystem capabilities and format versions.

## Limitations

- No OS integration/FUSE support.
- No journaling or crash recovery.
- No concurrent filesystem operations.
- No permissions support

## Roadmap

- [x] Basic block-device abstraction
- [x] Versioned filesystem format
- [x] Files and directories
- [x] Symbolic and hard links
- [x] Block caching
- [x] Integrity checking (partial, misses error correction)
- [ ] FUSE integration
- [ ] Journaling / crash recovery
- [ ] Improved allocation strategies
- [ ] Unit/integration test suite
- [ ] Concurrent access
- [ ] Permissions