use std::{
    fs::{DirEntry, ReadDir},
    path::PathBuf,
};

pub struct FsWalker {
    readers: Vec<ReadDir>,
    going_into_dir: Option<PathBuf>,
}

impl FsWalker {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            readers: vec![],
            going_into_dir: Some(dir),
        }
    }

    pub fn skip_incoming_dir(&mut self) -> Result<PathBuf, NoIncomingDir> {
        self.going_into_dir.take().ok_or(NoIncomingDir)
    }
}

impl Iterator for FsWalker {
    type Item = Result<DirEntry, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(going_into_dir) = self.going_into_dir.take() {
                let reader = std::fs::read_dir(&going_into_dir);

                match reader {
                    Err(err) => return Some(Err(err)),
                    Ok(reader) => {
                        self.readers.push(reader);
                        continue;
                    }
                }
            }

            let reader = self.readers.last_mut()?;

            let Some(entry) = reader.next() else {
                self.readers.pop();
                continue;
            };

            if let Ok(entry) = entry.as_ref() {
                if entry.path().is_dir() {
                    self.going_into_dir = Some(entry.path());
                }
            }

            return Some(entry);
        }
    }
}

#[derive(Debug)]
pub struct NoIncomingDir;
