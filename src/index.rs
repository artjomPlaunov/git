use core::panic;
use std::{
    cmp,
    collections::HashMap,
    fs::Metadata,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
};

use sha1::{digest::core_api::CoreWrapper, Digest, Sha1, Sha1Core};

use crate::lockfile::LockFile;

#[derive(Debug, Clone)]
pub struct Entry {
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
    pub flags: [u8; 2],
    pub path: String,
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

        // Just check executable bit for mode.
        let mode: [u8; 4] = if stat.permissions().mode() & 0o100 != 0 {
            // 0o100755 -> 0x81ED -> Big-endian padded with zero's.
            [0x00, 0x00, 0x81, 0xED]
        } else {
            // 0o100644 -> 0x81A4 -> Big-endian padded with zero's.
            [0x00, 0x00, 0x81, 0xA4]
        };

        let flag = cmp::min(0xFFF, pathname.len());

        Entry {
            ctime: stat.ctime().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting ctime."),
            ctime_nsec: stat.ctime_nsec().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting ctime_nsec."),
            mtime: stat.mtime().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting ctime."),
            mtime_nsec: stat.mtime_nsec().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting mtime_nsec."),
            dev: stat.dev().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting dev."),
            ino: stat.ino().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting ino."),
            mode: mode,
            uid: stat
                .uid()
                .to_be_bytes()
                .try_into()
                .expect("failure getting uid."),
            gid: stat
                .gid()
                .to_be_bytes()
                .try_into()
                .expect("failure getting gid."),
            size: stat.size().to_be_bytes()[4..8]
                .try_into()
                .expect("failure getting size."),
            oid: Vec::from(object_id),
            flags: flag.to_be_bytes()[6..8]
                .try_into()
                .expect("failure setting file size flag."),
            path: pathname,
        }
    }

    fn to_string(&self) -> String {
        let mut res: Vec<u8> = Vec::new();
        res.extend_from_slice(&self.ctime);
        res.extend_from_slice(&self.ctime_nsec);
        res.extend_from_slice(&self.mtime);
        res.extend_from_slice(&self.mtime_nsec);
        res.extend_from_slice(&self.dev);
        res.extend_from_slice(&self.ino);
        res.extend_from_slice(&self.mode);
        res.extend_from_slice(&self.uid);
        res.extend_from_slice(&self.gid);
        res.extend_from_slice(&self.size);
        res.extend_from_slice(&self.oid);
        res.extend_from_slice(&self.flags);
        res.extend_from_slice(&self.path.as_bytes());
        if res.len() % 8 == 0 {
            res.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        } else {
            while res.len() % 8 != 0 {
                res.push(0);
            }
        }
        let mut s = String::new();
        unsafe {
            s = String::from_utf8_unchecked(res);
        }
        return s;
    }
}

pub struct Index {
    keys: Vec<String>,
    entries: HashMap<String, Entry>,
    lockfile: LockFile,
    digest: CoreWrapper<Sha1Core>,
}

impl Index {
    pub fn new(path: PathBuf) -> Self {
        Self {
            keys: Vec::new(),
            entries: HashMap::new(),
            lockfile: LockFile::new(path),
            digest: Sha1::new(),
        }
    }

    pub fn each_entry(&mut self) -> Vec<Entry> {
        self.keys.sort();
        let mut entries = Vec::new();
        for k in &self.keys {
            let entry = self.entries.get(&k.clone()).unwrap();
            entries.push(entry.clone());
        }
        entries
    }

    pub fn add(&mut self, path: &PathBuf, object_id: &str, stat: Metadata) {
        let entry = Entry::new(path.clone(), object_id, stat);
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
        self.entries.insert(pathname.clone(), entry);
        self.keys.push(pathname);
    }

    pub fn write_updates(&mut self) -> bool {
        let lock_res = self.lockfile.hold_for_update();
        let bool = match lock_res {
            Ok(_) => true,
            Err(_) => false,
        };
        if !bool {
            return bool;
        }

        // hash index header
        let mut header: Vec<u8> = Vec::new();
        header.extend_from_slice(String::from("DIRC").as_bytes());
        let size: [u8; 4] = self.entries.len().to_be_bytes()[4..8]
            .try_into()
            .expect("failure getting ino.");
        header.extend_from_slice(&[0x00, 0x00, 0x00, 0x02]);
        header.extend_from_slice(&size);
        self.write(header);

        let mut data_vec = Vec::new();
        for entry in &mut self.each_entry() {
            let data = Vec::from(entry.clone().to_string().as_bytes());
            data_vec.push(data);
        }
        for data in data_vec {
            self.write(data);
        }
        self.finish_write();
        true
    }

    pub fn write(&mut self, data: Vec<u8>) {
        unsafe {
            let _ = self
                .lockfile
                .write(String::from_utf8_unchecked(data.clone()));
        }
        self.digest.update(&data);
    }

    pub fn finish_write(&mut self) {
        let hash_result = &self.digest.clone().finalize();
        let hash_result = hash_result.as_slice().to_vec();
        unsafe {
            let _ = self
                .lockfile
                .write(String::from_utf8_unchecked(hash_result.clone()));
        }
        let _ = self.lockfile.commit();
    }
}
