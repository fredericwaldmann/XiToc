use super::{Entry, Folder, Loaded};
use crate::{
    document::{DocumentData, Location, pdf_filename},
    pdf,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub struct Workspace {
    root: Dir,
    name: String,
}

impl Workspace {
    pub fn choose() -> Result<Option<Self>, String> {
        rfd::FileDialog::new()
            .set_title("Open XiToc workspace")
            .pick_folder()
            .map(Self::open)
            .transpose()
    }

    pub fn open(root: PathBuf) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let name = root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        Ok(Self {
            root: Dir::open_ambient_dir(root, ambient_authority()).map_err(|e| e.to_string())?,
            name,
        })
    }

    fn checked(&self, relative: &str) -> Result<PathBuf, String> {
        let path = Path::new(relative);
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("Path must remain inside the workspace".into());
        }
        let mut checked = PathBuf::new();
        for part in path.components() {
            checked.push(part);
            if self
                .root
                .symlink_metadata(&checked)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("Workspace symlinks are not followed".into());
            }
        }
        // All subsequent IO is relative to this capability; it cannot escape the
        // selected root even if another process races a path/symlink replacement.
        Ok(if checked.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            checked
        })
    }

    pub fn load(&self, relative: &str) -> Result<Loaded, String> {
        let path = self.checked(relative)?;
        let file = self.root.open(path).map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file()
            || !relative.to_ascii_lowercase().ends_with(".pdf")
        {
            return Err("Choose a PDF file".into());
        }
        let mut bytes = Vec::new();
        file.take(pdf::MAX_PDF_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let document = pdf::decode(&bytes, Location::new(relative.into()))?;
        Ok(Loaded {
            document,
            bytes: bytes.into(),
        })
    }

    pub fn create(&self, folder: &str, name: &str, data: DocumentData) -> Result<Loaded, String> {
        let parent = self
            .root
            .open_dir(self.checked(folder)?)
            .map_err(|e| e.to_string())?;
        let filename = pdf_filename(name)?;
        let bytes = pdf::encode(&data)?;
        write_new_in(&parent, &filename, |file| file.write_all(&bytes))?;
        let relative = if folder.is_empty() {
            filename
        } else {
            format!("{folder}/{filename}")
        };
        Ok(Loaded {
            document: crate::document::Document {
                data,
                location: Location::new(relative),
                external: false,
            },
            bytes: bytes.into(),
        })
    }

    pub fn scan(&self) -> Result<Folder, String> {
        self.scan_folder("", 0, &mut 4096)
    }

    fn scan_folder(
        &self,
        relative: &str,
        depth: usize,
        budget: &mut usize,
    ) -> Result<Folder, String> {
        if depth > 32 {
            return Err("Workspace exceeds 32 nested folders".into());
        }
        let path = self.checked(relative)?;
        let directory = self.root.open_dir(path).map_err(|e| e.to_string())?;
        let mut folder = Folder {
            path: relative.into(),
            name: if relative.is_empty() {
                self.name.clone()
            } else {
                relative.rsplit('/').next().unwrap_or(relative).into()
            },
            ..Default::default()
        };
        let mut children = directory
            .entries()
            .map_err(|e| e.to_string())?
            .take(*budget + 1)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if children.len() > *budget {
            return Err("Workspace exceeds 4096 entries; choose a smaller workspace".into());
        }
        children.sort_by_key(|e| e.file_name());
        for child in children {
            if *budget == 0 {
                return Err("Workspace exceeds 4096 entries; choose a smaller workspace".into());
            }
            *budget -= 1;
            let kind = child.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                continue;
            }
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| "Workspace filenames must be UTF-8")?;
            let child_path = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            if kind.is_dir() {
                folder
                    .folders
                    .push(self.scan_folder(&child_path, depth + 1, budget)?);
            } else if kind.is_file() && name.to_ascii_lowercase().ends_with(".pdf") {
                let (kind, document_id, problem) = match self.load(&child_path) {
                    Ok(loaded) => (
                        Some(loaded.document.data.kind()),
                        Some(loaded.document.data.id()),
                        None,
                    ),
                    Err(error) => (None, None, Some(error)),
                };
                folder.files.push(Entry {
                    path: child_path,
                    kind,
                    document_id,
                    problem,
                });
            }
        }
        Ok(folder)
    }
}

struct Temporary<'a> {
    directory: &'a Dir,
    name: String,
}
impl Drop for Temporary<'_> {
    fn drop(&mut self) {
        let _ = self.directory.remove_file(&self.name);
    }
}

/// Atomically publish complete, synced bytes, without ever replacing a file.
fn write_new_in(
    directory: &Dir,
    name: &str,
    write: impl FnOnce(&mut cap_std::fs::File) -> std::io::Result<()>,
) -> Result<(), String> {
    let name_of_temp = format!(".xitoc-{}.tmp", uuid::Uuid::new_v4());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = directory
        .open_with(&name_of_temp, &options)
        .map_err(|e| e.to_string())?;
    let temp = Temporary {
        directory,
        name: name_of_temp,
    };
    let written = write(&mut file).and_then(|()| file.sync_all());
    drop(file);
    written.map_err(|e| e.to_string())?;
    // Hard-link publication is atomic and fails if destination exists. Filesystems
    // without hard links return an error, preserving the original and removing temp.
    directory
        .hard_link(&temp.name, directory, name)
        .map_err(|e| e.to_string())?;
    directory
        .remove_file(&temp.name)
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    directory
        .open(".")
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn write_new(
        path: &Path,
        write: impl FnOnce(&mut cap_std::fs::File) -> std::io::Result<()>,
    ) -> Result<(), String> {
        let dir = Dir::open_ambient_dir(path.parent().unwrap(), ambient_authority()).unwrap();
        write_new_in(&dir, path.file_name().unwrap().to_str().unwrap(), write)
    }
    use crate::document::DocumentKind;
    #[test]
    fn creates_real_pdfs_reopens_after_restart_and_survives_move() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        let workspace = Workspace::open(dir.path().into()).unwrap();
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let name = kind.label();
            let created = workspace
                .create("nested", name, DocumentData::blank(kind))
                .unwrap();
            let restarted = Workspace::open(dir.path().into()).unwrap();
            assert_eq!(
                restarted
                    .load(created.document.location.name())
                    .unwrap()
                    .document
                    .data,
                created.document.data
            );
            fs::rename(
                dir.path().join(created.document.location.name()),
                dir.path().join(format!("{name}.pdf")),
            )
            .unwrap();
            assert_eq!(
                restarted
                    .load(&format!("{name}.pdf"))
                    .unwrap()
                    .document
                    .data,
                created.document.data
            );
        }
        let files = workspace.scan().unwrap();
        assert_eq!(files.files.len(), 2);
        assert_eq!(
            files
                .files
                .iter()
                .filter(|file| file.kind == Some(DocumentKind::Page))
                .count(),
            1
        );
        assert_eq!(
            files
                .files
                .iter()
                .filter(|file| file.kind == Some(DocumentKind::Whiteboard))
                .count(),
            1
        );
        assert!(files.folders[0].files.is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3); // folder + two PDFs, no sidecars
    }
    #[test]
    fn collisions_and_failed_writes_preserve_existing_bytes_and_clean_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("keep.pdf");
        fs::write(&destination, b"last valid revision").unwrap();
        assert!(write_new(&destination, |f| f.write_all(b"replacement")).is_err());
        assert!(
            write_new(&dir.path().join("fail.pdf"), |f| {
                f.write_all(b"partial")?;
                Err(std::io::Error::other("simulated disk failure"))
            })
            .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"last valid revision");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        let workspace = Workspace::open(dir.path().into()).unwrap();
        assert!(workspace.load("keep.pdf").is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"last valid revision");
    }
    #[test]
    fn rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(dir.path().into()).unwrap();
        for path in ["..", "../other", "/tmp"] {
            assert!(workspace.checked(path).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_and_ignores_links_in_tree() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
        let workspace = Workspace::open(dir.path().into()).unwrap();
        assert!(
            workspace
                .create("escape", "bad", DocumentData::blank(DocumentKind::Page))
                .is_err()
        );
        assert!(workspace.scan().unwrap().folders.is_empty());
        fs::create_dir(dir.path().join("raced")).unwrap();
        let checked = workspace.checked("raced").unwrap();
        fs::remove_dir(dir.path().join("raced")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("raced")).unwrap();
        assert!(workspace.root.open_dir(checked).is_err());
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }
}
