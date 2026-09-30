# FerrisFileSystem Version 1 On-Disk Format

This document specifies the disk representation currently written and read by `FormatV1`. It is intended for implementers of image-inspection, validation, or compatibility tools. Numbers in this document are decimal unless prefixed with `0x`.

Version 1 uses a fixed block size of **256 bytes** and little-endian encoding for every multi-byte integer.

> **Compatibility scope.** The format is defined by the current Rust implementation. Several fields are reserved or only partially validated, and there is no crash-recovery protocol. A writer that needs exact compatibility should follow the rules below and preserve unknown/reserved bytes where possible.

## Terms and constants

| Term | Value / meaning |
| --- | --- |
| Block size (`B`) | 256 bytes |
| Block number | A zero-based, device-wide block address |
| Data start | Block number indicating the start of region where data is stored |
| Data index | A zero-based index relative to `data_start`; its device block number is `data_start + data_index` |
| Inode index | A zero-based index in the inode table |
| Invalid address | `0xffffffff` (`u32::MAX`) |
| Format magic | `0x2e2e616e616e6162` |
| Format version | `1` |
| Byte order | Little-endian for `u16`, `u32`, and `u64` |
| Inodes per block | 2 (`floor(B / size_of_inode)`) |
| Bitmap coverage | 2,048 indexed items per bitmap block (`256 × 8`) |

The magic bytes at offsets `0x00..0x08`, in on-disk byte order, are `62 61 6e 61 6e 61 2e 2e`. The invalid-address value is used for unused inode pointers and for a free directory region's inode field.

## Whole-image layout

The image consists of `total_blocks` consecutive 256-byte blocks. All metadata regions are contiguous and are described by the superblock.

```text
         Region            |   Block Number
┌──────────────────────────┐ <-  0
│ Superblock               │
├──────────────────────────┤ <-  inode_bitmap_start
│ Inode Allocation Bitmap  │
├──────────────────────────┤ <-  block_bitmap_start
│ Data Allocation Bitmap   │
├──────────────────────────┤ <-  inode_table_start
│ Inode Table              │
├──────────────────────────┤ <-  data_start
│ Data Region              │
└──────────────────────────┘
```

For an image written by the current formatter, the region starts are derived as follows:

```text
inode_bitmap_start = 1
block_bitmap_start = inode_bitmap_start + inode_bitmap_blocks
inode_table_start  = block_bitmap_start + block_bitmap_blocks
data_start         = inode_table_start + inode_table_blocks
```

Consequently, the data region contains `total_blocks - data_start` blocks. The formatter calculates its region sizes from the device's block count `N`:

```text
requested_inode_count = ceil((256 × N) / 1024) = ceil(N / 4)
inode_table_blocks    = ceil(requested_inode_count / 2)
inode_bitmap_blocks   = ceil(inode_table_blocks / 2048)
block_bitmap_blocks   = ceil(N / 2048)
```

The usable inode capacity is `inode_table_blocks × inodes_per_block`. An inode record is 96 bytes, meaning that each inode-table block has room for two 96-byte records. The usable data-block capacity is `total_blocks - data_start`. Bitmap bits that lie beyond either capacity have no meaning and must be ignored by readers.

The formatter creates root directory inode `0`, associated to data block at index `0`. The root directory therefore resides at device block `data_start`.

## Superblock

The superblock occupies device block 0. The serialized payload is 56 bytes; bytes `56..255` are outside the serialized structure and should be treated as reserved. The current writer does not explicitly clear those bytes before serializing, so readers must not assign them meaning.

| Byte range | Size | Type | Field | Meaning |
| --- | ---: | --- | --- | --- |
| `0x00..0x08` | 8 | `u64` | `magic` | Must be `0x2e2e616e616e6162`. |
| `0x08..0x0c` | 4 | `u32` | `version` | Must be `1`. |
| `0x0c..0x10` | 4 | `u32` | `block_size` | Writer records `256`. The current implementation uses 256 regardless of the field value. |
| `0x10..0x14` | 4 | `u32` | `total_blocks` | Number of blocks in the image. |
| `0x14..0x18` | 4 | `u32` | `inode_bitmap_start` | Device block where the inode bitmap begins. |
| `0x18..0x1c` | 4 | `u32` | `inode_bitmap_blocks` | Length of the inode bitmap region, in blocks. |
| `0x1c..0x20` | 4 | `u32` | `block_bitmap_start` | Device block where the data bitmap begins. |
| `0x20..0x24` | 4 | `u32` | `block_bitmap_blocks` | Length of the data bitmap region, in blocks. |
| `0x24..0x28` | 4 | `u32` | `inode_table_start` | Device block where the inode table begins. |
| `0x28..0x2c` | 4 | `u32` | `inode_table_blocks` | Length of the inode table, in blocks. |
| `0x2c..0x30` | 4 | `u32` | `data_start` | First device block in the data region. |
| `0x30..0x34` | 4 | `u32` | `root_inode` | Inode index of the root directory; current formatter writes `0`. |
| `0x34..0x38` | 4 | `u32` | `free_inodes` | Cached count of clear bits in the usable inode bitmap range. |
| `0x38..0x3c` | 4 | `u32` | `free_data` | Cached count of clear bits in the usable data bitmap range. |

A robust reader should verify the magic, version, fixed block size, non-overlapping region bounds, and that `data_start <= total_blocks`.

TODO: The current mount path validates the magic but does not yet comprehensively validate the geometry before using it.

## Allocation bitmaps

The inode and data bitmaps have the same representation. A bit value of `1` means allocated; `0` means free. Bitmap bit `i` is stored as follows:

```text
bitmap device block = bitmap_start + floor(i / 2048)
byte within block   = floor(i / 8) mod 256
bit within byte     = i mod 8
mask                = 1 << bit_within_byte
```

Bit zero is the least significant bit of byte zero. Bitmap indexes map directly to inode indexes for the inode bitmap and to data indexes for the data bitmap (this means relative to the corresponding region start). A bitmap never addresses device absolute block numbers directly.

The allocator searches from its last allocation position and wraps to the beginning if required. This is an allocation policy, not an on-disk requirement. It only considers indexes below the region capacity. The unused tail bits in the final bitmap block may contain either value and must not affect free-space calculations or validation.

`free_inodes` and `free_data` are cached metadata, not authoritative allocation maps. They should respectively equal:

```text
(inode_table_blocks × 2) - allocated_inode_bits
(total_blocks - data_start) - allocated_data_bits
```

where the bit counts are restricted to the usable index ranges.

## Inode table

An inode record has a fixed serialized size of 96 bytes. Two records are stored per 256-byte inode-table block, at offsets 0 and 96. The final 64 bytes of every inode-table block, offsets `192..255`, are unused by the current format.

For inode index `i`:

```text
device block        = inode_table_start + floor(i / 2)
offset within block = (i mod 2) × 96
```

The allocation bitmap determines whether an inode slot is in use. Contents of a free slot are not required to be zeroed and must not be treated as a valid inode.

### Inode record

All offsets in this table are relative to the start of the 96-byte record.

| Byte range | Size | Type | Field | Meaning |
| --- | ---: | --- | --- | --- |
| `0x00..0x08` | 8 | `u64` | `size` | Logical content size in bytes. |
| `0x08` | 1 | `u8` | `file_type` | `0` regular file, `1` directory, `2` symbolic link, other values unknown/invalid. |
| `0x09` | 1 | `u8` | padding | Writer stores zero; ignored by reader. |
| `0x0a..0x0c` | 2 | `u16` | `permissions` | Currently written as `0xffff` for newly created objects. No access-control behavior is implemented. |
| `0x0c..0x0e` | 2 | `u16` | `links` | Stored link count. For regular files, it is incremented for each hard link and does not include symbolic-link targets. Directory link accounting is currently limited to its creation value. |
| `0x0e..0x10` | 2 | `u16` | padding | Writer stores zero; ignored by reader. |
| `0x10..0x14` | 4 | `u32` | `blocks` | Number of logical content data blocks addressed by this inode. It excludes indirect pointer blocks. |
| `0x14..0x44` | 48 | `u32[12]` | `direct` | Twelve data indexes, in logical block order. |
| `0x44..0x48` | 4 | `u32` | `indirect` | Data index of a single-indirect pointer block, or `0xffffffff`. |
| `0x48..0x4c` | 4 | `u32` | `double` | Data index of a double-indirect pointer block, or `0xffffffff`. |
| `0x4c..0x54` | 8 | `u64` | `created` | Seconds since the Unix epoch. |
| `0x54..0x5c` | 8 | `u64` | `modified` | Seconds since the Unix epoch. |
| `0x5c..0x60` | 4 | — | unused record tail | Not serialized; should be ignored. |

All addresses in `direct`, `indirect`, `double`, and pointer blocks are data indexes. Convert any non-invalid value `a` to a device block with `data_start + a`.

The inode's content capacity is `blocks × 256` bytes. A valid inode must have `size <= blocks × 256`. Empty regular files have `blocks = 0` and `size = 0`; a newly created directory has one content block and `size = 256`; a symbolic link is allocated one content block on creation and its `size` is the target-path byte length.

### Content block addressing

Each pointer block is a 256-byte array of 64 little-endian `u32` data indexes. Only entries needed to address logical content blocks are significant.

| Logical content-block index `n` | Address source |
| --- | --- |
| `0 <= n < 12` | `direct[n]` |
| `12 <= n < 76` | pointer block at `indirect`; entry `n - 12` |
| `76 <= n < 4172` | double pointer block at `double`; entry `floor((n - 76) / 64)` gives an indirect pointer block; that block's entry `(n - 76) mod 64` gives the content block |

The maximum logical block count is `12 + 64 + 64 × 64 = 4172`, so the maximum logical size is **1,068,032 bytes** (`4172 × 256`). This limit is independent of the `u64` width of `size`.

Indirect and double-indirect pointer blocks are allocated from the normal data region and are marked in the data bitmap. They are metadata blocks, not logical content blocks, and therefore are excluded from the inode's `blocks` field. The first transition beyond direct pointers allocates one single-indirect block. The first double-indirect content block allocates one double-indirect block and one child indirect block; each additional group of 64 blocks requires another child indirect block.

## Directories

A directory is an inode whose logical content blocks are directory blocks. Its `size` is the number of directory blocks multiplied by 256. Every directory block is a sequence of variable-length records that must exactly fill the block.

### Directory record format

At each record boundary, read `inode` and `record_len`. If `inode == 0xffffffff`, the record represents a free region. Otherwise it is an allocated directory entry.

| Byte range | Size | Type | Present for | Field |
| --- | ---: | --- | --- | --- |
| `0x00..0x04` | 4 | `u32` | all records | `inode`: inode index, or `0xffffffff` for free space |
| `0x04..0x06` | 2 | `u16` | all records | `record_len`: total record/free-region length in bytes |
| `0x06..0x08` | 2 | `u16` | allocated records | `name_len`: length in bytes |
| `0x08` | 1 | `u8` | allocated records | `file_type`: same encoding as inode `file_type` |
| `0x09..` | `name_len` | bytes | allocated records | name bytes |
| remaining bytes | variable | — | allocated records | zero-filled padding up to `record_len` |

Allocated entry length is always the four-byte-aligned size:

```text
record_len = align_up(9 + name_len, 4)
```

The largest name the current writer creates is 64 bytes. For such a name, the record length is 76 bytes. Names are taken from Rust path strings, so the current creation API accepts UTF-8 names; the record format itself stores raw bytes. A compatibility reader should regard `name_len > 64` as invalid for version 1, and should not assume zero padding is part of the name.

For a free region, only the first six bytes have a defined representation. Its minimum representable length is eight bytes (`align_up(6, 4)`). The bytes after the record length are cleared by the writer but have no semantic fields. Free regions may occur at the end of a block or after deletion. Adjacent free regions are coalesced by the implementation and are reported by the integrity checker as an undesirable structure.

Directory parsing begins at byte offset zero of each logical directory block and advances by `record_len`. A valid block has all of these properties:

- Every `record_len` is non-zero.
- A record never extends past byte 256.
- The sum of record lengths is exactly 256.
- Allocated entries use the minimum aligned length for their `name_len`.
- Allocated records reference a valid allocated inode, and their `file_type` agrees with that inode's type.

The first two allocated entries of a newly created directory are:

| Name | Inode | Type |
| --- | --- | --- |
| `.` | the directory's own inode | directory |
| `..` | the parent inode; root points to itself | directory |

The writer initially leaves one free record covering the remainder of the first block. It reuses a free record when it is exactly the required size or leaves a remainder of at least eight bytes; otherwise it allocates a new directory data block.

## Files and links

A regular file's content is the first `size` bytes of the sequence of logical content blocks addressed by its inode. Writes beyond the end allocate enough logical data blocks to cover the requested end position and zero newly allocated data blocks before use. Holes created by seeking past EOF are physically allocated and contain zero bytes until written.

A symbolic link stores its target as the first `size` bytes of its first direct data block. The current writer allocates exactly one block for a symbolic link, so a target must fit in 256 bytes. An empty target is rejected during resolution. A target whose first byte is `/` is resolved from the root; any other target is resolved relative to the symlink's parent directory. Resolution follows at most 40 symbolic links.

A hard link is another allocated directory entry referring to the same **regular-file** inode. It increments that inode's `links` field; a symlink is its own inode and does not increment its target's count. Deleting an entry decrements `links`; when it reaches zero, the implementation deallocates the inode and its referenced logical content blocks. Pointer-block reclamation is not currently included in that deletion path. A directory may only be deleted after all entries other than `.` and `..` have been removed. The root inode cannot be deleted.

## Formatter initialization and persistence properties

Formatting writes the superblock, root inode, root directory block, and allocation bits for the root inode/data block. It clears bitmap blocks other than the root-containing first bitmap blocks before setting root bit zero. It does not define a secure-wipe guarantee for free inode-table slots or free data blocks; consumers must rely on allocation metadata rather than expecting unused bytes to be zero.

There is no journal, copy-on-write scheme, checksum, transaction log, or ordering barrier in version 1. A failed or interrupted modification can leave directory entries, inode references, allocation bitmaps, and free counts out of sync. The built-in integrity checker can detect several such conditions, including malformed records, invalid addresses, unallocated references, duplicate data references, unreachable allocated data, and mismatched free counts. It reports findings but does not repair the image.

Version 1 is designed for serialized access. Concurrent mutation is unsupported.

## Validation checklist

An image validator should, at minimum:

1. Check image length, magic, version, fixed block size, and all region bounds.
2. Derive inode/data capacities from the region geometry and ignore bitmap tail bits.
3. Ensure the root inode index is within the inode capacity, allocated, and of type directory.
4. Check every allocated inode's type, `size <= blocks × 256`, logical block count, and all required pointers.
5. Verify that each referenced content or pointer block is within the data capacity and marked allocated.
6. Parse every reachable directory block to exactly 256 bytes; validate entry lengths, names, inode references, and inode/type agreement.
7. Traverse from root through directory entries, detecting multiply referenced data blocks and allocated inodes/data blocks that are unreachable.
8. Recount usable bitmap bits and compare the result with `free_inodes` and `free_data`.

When generating images, write all numeric values little-endian, keep all addresses data-region-relative where an inode or pointer block stores them, and update the relevant bitmap plus the cached free count whenever allocating or releasing a resource.
