pub mod format;
pub mod superblock;
pub mod bitmap_allocator;
pub mod inode_handler;
pub mod inode;
pub mod directory;
pub mod directory_handler;

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