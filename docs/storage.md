# Storage Devices

The physical location of the storage is abstracted, allowing the rest of the logic to be independent of the underlying storage.
The following sections describe the currently supported storage backends.

### `BlockDevice`

`BlockDevice` defines the common interface used by the filesystem for block storage, including:

* Block reads.
* Block writes.
* Block-count queries.
* Resizing.
* Flushing pending data.

This abstraction allows the filesystem implementation to operate independently of the underlying storage mechanism.

### `MemoryDevice`

`MemoryDevice` stores the filesystem image in memory and supports importing from and exporting to disk-image files.

It is primarily useful for testing, demonstrations, and filesystem development.

### `FileDevice`

`FileDevice` provides direct block access to a host file and supports resizing the backing image.

It allows the filesystem to operate directly on a persistent disk-image file.

### `CachedDevice`

`CachedDevice<D>` provides an LRU write-back cache over another `BlockDevice`.

Writes are initially applied to cached blocks and marked dirty. Dirty blocks are written back to the underlying device when evicted or when the cache is flushed.