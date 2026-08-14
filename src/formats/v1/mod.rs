pub mod format;
pub mod file;
pub mod superblock;
pub mod bitmap_allocator;
pub mod inode_handler;
pub mod inode;
pub mod directory;
pub mod directory_handler;
pub mod integrity_checker_errors;

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