# Errors

## Error Handling

Filesystem operations return `FSResult<T>`, which in case of error contains an instance of `FSError`.

`FSError` represents failures such as:

* Invalid paths.
* Invalid filesystem object types.
* Resource exhaustion.
* Invalid input.
* Filesystem corruption.
* I/O failures.

`FSErrorCode` provides stable numeric codes for these error categories, including dedicated corruption-related codes.


## Filesystem Integrity

Version 1 includes an integrity checker that traverses filesystem metadata and directory structures to detect inconsistencies in the on-disk state.

Checks include:

* Allocated but unreachable inodes or data blocks.
* References to unallocated blocks.
* Multiply referenced blocks where not permitted.
* Invalid inode pointers.
* Malformed directory records.
* Invalid directory structures.
* File-type inconsistencies.
* Inconsistent allocation and free-space metadata.

Integrity findings are represented through the format-independent `IntegrityResult` and `IntegrityError` interfaces.

TODO: merge `FSError` with `IntegrityError` and add error recovery / correction