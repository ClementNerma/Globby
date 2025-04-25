use std::{cmp::Ordering, fs::DirEntry, path::PathBuf};

/// A walker that yields items recursively from a provided base directory
pub struct FsWalker {
    queue: Vec<Vec<Result<DirEntry, std::io::Error>>>,
    going_into_dir: Option<PathBuf>,
}

impl FsWalker {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            queue: vec![],
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
            // Check if we're going into a directory
            if let Some(going_into_dir) = self.going_into_dir.take() {
                match std::fs::read_dir(&going_into_dir) {
                    Err(err) => return Some(Err(err)),
                    Ok(items_iter) => {
                        let mut items = items_iter.collect::<Vec<_>>();

                        // Sort items so that errors appear at the end,
                        // and correct directory entries are in descending alphabetic order
                        // This is so the queue can be .pop()ed later to get the "earliest" items later on
                        items.sort_by(|a, b| match (a, b) {
                            (Ok(a), Ok(b)) => b.file_name().cmp(&a.file_name()),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        });

                        self.queue.push(items);

                        continue;
                    }
                }
            }

            // Otherwise, get the currently handled directory's entries
            let queue = self.queue.last_mut()?;

            let Some(entry) = queue.pop() else {
                // If the reader is empty, remove it from the last
                self.queue.pop();
                // then get to use the next reader
                continue;
            };

            // If the sub-iterator didn't yield an error...
            if let Ok(entry) = entry.as_ref() {
                // Check if we're going into a directory
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
