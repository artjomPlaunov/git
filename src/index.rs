use core::panic;
use std::{cmp, fs::Metadata, os::unix::fs::{MetadataExt, PermissionsExt}, path::PathBuf};





#[derive(Debug, Clone)]
pub struct Entry {
    /*
    pub max_path_size: [u8; 4],
    */
    pub ctime: [u8; 4],
    pub ctime_nsec: [u8; 4],
    pub mtime: [u8; 4],
    pub mtime_nsec: [u8; 4],
    pub dev: [u8; 4],
    pub ino: [u8; 4],
    pub mode: [u8; 4],
    pub uid: [u8; 4],
    pub gid: [u8; 4],
    pub size: [u8; 4],
    pub oid: Vec<u8>,
    pub flags: [u8; 4],
    pub path: Vec<u8>
}

impl Entry {
    pub fn new(path: PathBuf, object_id: &str, stat: Metadata) -> Self {
        
        let mut pathname = String::new();
        match path.to_str() {
            Some(s) => {
                pathname = String::from(s);
            }
            None => {
                eprintln!("Error reading pathname.");
                panic!();
            }
        }

        let mode: [u8; 4] = if stat.permissions().mode() & 0o100 != 0 {
            // 0o100755 -> 0x81ED -> Big-endian padded with zero's.
            [0x00, 0x00, 0x81, 0xED]
        } else {
            // 0o100644 -> 0x81A4 -> Big-endian padded with zero's. 
            [0x00, 0x00, 0x81, 0xA4]
        };

        let path = Vec::from(pathname.as_bytes());

        let flag = cmp::min(0xFFF, path.len());

        Entry {
            ctime: stat.ctime().to_be_bytes()[4..8].try_into().expect("failure getting ctime."),
            ctime_nsec: stat.ctime_nsec().to_be_bytes()[4..8].try_into().expect("failure getting ctime_nsec."),
            mtime: stat.mtime().to_be_bytes()[4..8].try_into().expect("failure getting ctime."),
            mtime_nsec: stat.mtime_nsec().to_be_bytes()[4..8].try_into().expect("failure getting mtime_nsec."),
            dev: stat.dev().to_be_bytes()[4..8].try_into().expect("failure getting dev."),
            ino: stat.ino().to_be_bytes()[4..8].try_into().expect("failure getting ino."),
            mode: mode,
            uid: stat.uid().to_be_bytes().try_into().expect("failure getting uid."),
            gid: stat.gid().to_be_bytes().try_into().expect("failure getting gid."),
            size: stat.size().to_be_bytes()[4..8].try_into().expect("failure getting size."),
            oid: Vec::from(object_id),
            flags: flag.to_be_bytes()[4..8].try_into().expect("failure setting file size flag."),
            path,
        }
    }
}