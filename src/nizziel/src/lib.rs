pub mod blocking;

mod r#async;
pub use r#async::*;

mod error;
pub use error::Error;

use std::path::{ Path, PathBuf };
use std::fs::{ create_dir_all, File };

#[derive(Clone, Default, Debug)]
pub struct Downloads {
    pub downloads: Vec<Download>,
    pub retries: u8,
}

#[derive(Clone, Default, Debug)]
pub struct Download {
    pub url: String,
    pub path: PathBuf,
    pub unzip: bool,
}

pub fn create_file(path: &Path, read: bool, write: bool) -> std::io::Result<File> {
    let mut file = std::fs::OpenOptions::new()
        .read(read)
        .write(write)
        .open(path);

    if let Err(_) = file {
        //handle non-existence of parent()
        create_dir_all(&path.parent().unwrap())?;

        file = std::fs::OpenOptions::new()
            .read(read)
            .write(write)
            .create(true)
            .open(path);
    }

    file
}