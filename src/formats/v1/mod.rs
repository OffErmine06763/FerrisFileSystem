pub mod format;
pub mod superblock;
pub mod bitmap_allocator;
pub mod inode_handler;
pub mod inode;

/*
-----------------
|  superblock   |
-----------------
| inode bitmap  |
-----------------
| block bitmap  |
-----------------
|    inodes     |
-----------------
|     data      |
-----------------
*/