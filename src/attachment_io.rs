//! Explicit local attachment I/O. Results return to the owning canvas only.
use crate::{
    asset::BinaryAsset,
    attachment::{Attachment, original_name},
    image_import::Target,
};
use std::sync::{Arc, mpsc};

type FileData = (String, Arc<BinaryAsset>);

pub enum ResultEvent {
    Import(
        Target,
        Option<uuid::Uuid>,
        Result<FileData, String>,
        bool,
        Option<bool>,
    ),
    PickerClosed,
    Done(String),
}

pub struct AttachmentIo {
    tx: mpsc::Sender<ResultEvent>,
    pub rx: mpsc::Receiver<ResultEvent>,
    pub pending: usize,
    pub picker_open: bool,
}

impl Default for AttachmentIo {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            pending: 0,
            picker_open: false,
        }
    }
}

fn loaded(name: &str, bytes: Result<Arc<[u8]>, String>) -> Result<FileData, String> {
    let name = original_name(name).ok_or("Invalid attachment filename.")?;
    Ok((name.to_owned(), BinaryAsset::new(bytes?)?))
}

fn send(tx: &mpsc::Sender<ResultEvent>, ctx: &egui::Context, event: ResultEvent) {
    let _ = tx.send(event);
    ctx.request_repaint();
}

impl AttachmentIo {
    pub fn pick(
        &mut self,
        ctx: egui::Context,
        target: Target,
        scope: Option<uuid::Uuid>,
        listing: Option<bool>,
    ) {
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        let tx = self.tx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new();
            let paths = if target.replace.is_some() || listing.is_some() {
                dialog.pick_file().into_iter().collect()
            } else {
                dialog.pick_files().unwrap_or_default()
            };
            let extra = paths.len() > 4;
            for (index, path) in paths.into_iter().take(4).enumerate() {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                send(
                    &tx,
                    &ctx,
                    ResultEvent::Import(
                        offset(target, index),
                        scope,
                        loaded(name, crate::image_import::read_path(&path)),
                        false,
                        listing,
                    ),
                );
            }
            finish_picker(&tx, &ctx, extra);
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let dialog = rfd::AsyncFileDialog::new();
            let files = if target.replace.is_some() || listing.is_some() {
                dialog.pick_file().await.into_iter().collect()
            } else {
                dialog.pick_files().await.unwrap_or_default()
            };
            let extra = files.len() > 4;
            for (index, file) in files.into_iter().take(4).enumerate() {
                let result = loaded(
                    &file.file_name(),
                    crate::image_import::read_file(file.inner()).await,
                );
                send(
                    &tx,
                    &ctx,
                    ResultEvent::Import(offset(target, index), scope, result, false, listing),
                );
            }
            finish_picker(&tx, &ctx, extra);
        });
    }

    pub fn drop_file(
        &mut self,
        ctx: egui::Context,
        target: Target,
        scope: Option<uuid::Uuid>,
        file: egui::DroppedFileHandle,
    ) {
        if self.pending >= 4 {
            send(
                &self.tx,
                &ctx,
                ResultEvent::Done(
                    "Attachment queue is full. Drop remaining files after this batch.".into(),
                ),
            );
            return;
        }
        self.pending += 1;
        let tx = self.tx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let path = file.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let result = loaded(name, crate::image_import::read_path(path));
            send(
                &tx,
                &ctx,
                ResultEvent::Import(target, scope, result, true, None),
            );
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let result = if let Some(file) = file.web_file() {
                loaded(&file.name(), crate::image_import::read_file(file).await)
            } else {
                Err("Browser did not supply the dropped file.".into())
            };
            send(
                &tx,
                &ctx,
                ResultEvent::Import(target, scope, result, true, None),
            );
        });
    }

    pub fn save(&self, ctx: egui::Context, attachment: Arc<Attachment>) {
        let tx = self.tx.clone();
        let name = export_name(&attachment.original_name);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new().set_file_name(name).save_file() {
                let result = native::save_new(&path, attachment.asset.bytes())
                    .map(|()| "Attachment exported.".into())
                    .unwrap_or_else(|error| error);
                send(&tx, &ctx, ResultEvent::Done(result));
            }
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_file_name(name)
                .save_file()
                .await
            {
                let result = file
                    .write(attachment.asset.bytes())
                    .await
                    .map(|()| "Attachment download requested.".to_owned())
                    .unwrap_or_else(|error| format!("Download failed: {error}"));
                send(&tx, &ctx, ResultEvent::Done(result));
            }
        });
    }

    pub fn open(&self, ctx: egui::Context, attachment: Arc<Attachment>) {
        #[cfg(target_arch = "wasm32")]
        self.save(ctx, attachment);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let result = native::open(&attachment)
                    .map(|()| "Attachment handed to its default application.".into())
                    .unwrap_or_else(|error| error);
                send(&tx, &ctx, ResultEvent::Done(result));
            });
        }
    }
}

fn finish_picker(tx: &mpsc::Sender<ResultEvent>, ctx: &egui::Context, extra: bool) {
    send(tx, ctx, ResultEvent::PickerClosed);
    if extra {
        send(tx, ctx, ResultEvent::Done("Only the first four files were imported. Attach any remaining files in another batch.".into()));
    }
}

fn offset(mut target: Target, index: usize) -> Target {
    target.position = [
        target.position[0] + 16. * index as f64,
        target.position[1] + 40. * index as f64,
    ];
    target
}

fn export_name(name: &str) -> String {
    // Used only as a dialog suggestion. Extraction uses a generated basename.
    name.chars()
        .map(|c| {
            if matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        sync::Mutex,
        time::Duration,
    };

    const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
    static EXTRACTION: Mutex<()> = Mutex::new(());

    fn allowed_extension(attachment: &Attachment) -> Result<String, String> {
        let ext = attachment.extension();
        let bytes = attachment.asset.bytes();
        let safe = match ext.as_str() {
            "pdf" => bytes.starts_with(b"%PDF-"),
            "xlsx" | "ods" | "docx" | "odt" | "zip" => {
                bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
            }
            "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "jpeg" | "jpg" => bytes.starts_with(b"\xff\xd8\xff"),
            "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            "txt" | "csv" | "md" | "json" | "rs" | "c" | "cpp" | "tex" => {
                std::str::from_utf8(bytes).is_ok_and(|text| {
                    !text
                        .trim_start_matches('\u{feff}')
                        .trim_start()
                        .starts_with("#!")
                        && !text.contains('\0')
                })
            }
            _ => false,
        };
        if safe {
            Ok(ext)
        } else {
            Err("Direct opening is blocked for this file type or content. Use Save As to inspect or open it separately.".into())
        }
    }

    fn root() -> Result<PathBuf, String> {
        let directory = eframe::storage_dir("XiToc")
            .ok_or("No user storage for attachment opening.")?
            .join("attachment-open");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        if !fs::symlink_metadata(&directory)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_dir()
        {
            return Err("Attachment extraction directory is invalid.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        Ok(directory)
    }

    fn extract(
        root: &Path,
        ext: &str,
        bytes: &[u8],
    ) -> Result<(tempfile::TempDir, PathBuf), String> {
        let dir = tempfile::Builder::new()
            .prefix("open-")
            .tempdir_in(root)
            .map_err(|e| e.to_string())?;
        let path = dir.path().join(format!("attachment.{ext}"));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        Ok((dir, path))
    }

    pub fn save_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or("Choose a destination directory.")?;
        let mut file = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| format!("Cannot export attachment: {e}"))?;
        file.write_all(bytes)
            .and_then(|()| file.as_file().sync_all())
            .map_err(|e| format!("Cannot export attachment: {e}"))?;
        file.persist_noclobber(path)
            .map_err(|e| format!("Cannot create the attachment export: {}", e.error))?;
        Ok(())
    }

    pub fn open(attachment: &Attachment) -> Result<(), String> {
        open_in(&root()?, attachment, launch)
    }

    fn open_in(
        dir: &Path,
        attachment: &Attachment,
        launch: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let ext = allowed_extension(attachment)?;
        let (temp, path) = {
            let _guard = EXTRACTION
                .lock()
                .map_err(|_| "Attachment extraction is unavailable.")?;
            let mut count = 0;
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
                if !entry.file_name().to_string_lossy().starts_with("open-")
                    || !entry.file_type().is_ok_and(|kind| kind.is_dir())
                {
                    continue;
                }
                let old = entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > TTL);
                if !old || fs::remove_dir_all(entry.path()).is_err() {
                    count += 1;
                }
            }
            if count >= 64 {
                return Err("Temporary attachment limit reached. Use Save As.".into());
            }
            extract(dir, &ext, attachment.asset.bytes())?
        };
        launch(&path).map_err(|error| format!("Cannot open attachment: {error}. Use Save As."))?;
        // Keep the snapshot available after the opener exits and across XiToc restarts.
        let _ = temp.keep();
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn launch(path: &Path) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        const COMMAND: &str = "/usr/bin/open";
        #[cfg(not(target_os = "macos"))]
        const COMMAND: &str = "xdg-open";
        let status = std::process::Command::new(COMMAND)
            .arg(path)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("default application returned {status}"))
        }
    }

    #[cfg(target_os = "windows")]
    fn launch(path: &Path) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                window: *mut std::ffi::c_void,
                operation: *const u16,
                file: *const u16,
                arguments: *const u16,
                directory: *const u16,
                show: i32,
            ) -> isize;
        }
        let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        let verb: Vec<_> = "open".encode_utf16().chain([0]).collect();
        // Both strings live for the call and are NUL-terminated. No command-line arguments.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result > 32 {
            Ok(())
        } else {
            Err(format!("Windows default application error {result}"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn extracts_without_collisions_and_exports_without_clobbering() {
            let root = tempfile::tempdir().unwrap();
            let (_a, a) = extract(root.path(), "txt", b"first").unwrap();
            let (_b, b) = extract(root.path(), "txt", b"second").unwrap();
            assert_ne!(a, b);
            assert_eq!(fs::read(&a).unwrap(), b"first");
            let destination = root.path().join("report.txt");
            save_new(&destination, b"original").unwrap();
            assert!(save_new(&destination, b"overwrite").is_err());
            assert_eq!(fs::read(&destination).unwrap(), b"original");
        }
        #[test]
        fn dispatch_is_explicit_and_missing_handlers_report_an_error() {
            let root = tempfile::tempdir().unwrap();
            let file = Attachment::new(
                uuid::Uuid::new_v4(),
                crate::drawing::Surface::Canvas,
                [0.; 2],
                "résumé.pdf".into(),
                BinaryAsset::new(b"%PDF-1.7".as_slice().into()).unwrap(),
            );
            let mut captured = None;
            open_in(root.path(), &file, |path| {
                assert_eq!(path.file_name().unwrap(), "attachment.pdf");
                assert_eq!(fs::read(path).unwrap(), file.asset.bytes());
                captured = Some(path.to_owned());
                Ok(())
            })
            .unwrap();
            assert!(captured.unwrap().is_file());
            let error = open_in(root.path(), &file, |_| Err("no application".into())).unwrap_err();
            assert!(error.contains("no application"));
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            let mut unsupported = file;
            unsupported.original_name = "danger.exe".into();
            assert!(open_in(root.path(), &unsupported, |_| panic!("must not dispatch")).is_err());
        }

        #[test]
        fn executable_and_mislabeled_data_are_export_only() {
            for (name, data, safe) in [
                ("report.pdf", b"%PDF-".as_slice(), true),
                ("bad.txt", b"#!/bin/sh", false),
                ("tool.exe", b"MZ", false),
                ("bad.pdf", b"MZ", false),
            ] {
                let attachment = Attachment::new(
                    uuid::Uuid::new_v4(),
                    crate::drawing::Surface::Canvas,
                    [0.; 2],
                    name.into(),
                    BinaryAsset::new(data.into()).unwrap(),
                );
                assert_eq!(allowed_extension(&attachment).is_ok(), safe);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_formats_and_empty_files_preserve_bytes() {
        let fixtures: [(&str, &[u8]); 7] = [
            ("paper.pdf", b"%PDF-1.7"),
            ("sheet.xlsx", b"PK\x03\x04"),
            ("table.csv", b"a,b\r\n"),
            ("archive.zip", b"PK\x05\x06"),
            ("note.txt", b"\xef\xbb\xbfHello"),
            ("data.unknown", b"\0\xff\x99"),
            ("empty", b""),
        ];
        for (name, bytes) in fixtures {
            let (actual, asset) = loaded(name, Ok(bytes.into())).unwrap();
            assert_eq!(name, actual);
            assert_eq!(asset.bytes(), bytes);
            assert_eq!(
                asset.id(),
                <[u8; 32]>::from(<sha2::Sha256 as sha2::Digest>::digest(bytes))
            );
        }
        assert!(loaded("x.csv", Err("permission denied".into())).is_err());
        assert!(loaded("..", Ok(Arc::from([]))).is_err());
        assert!(
            loaded(
                "oversized",
                Ok(vec![0; crate::asset::MAX_ASSET_BYTES + 1].into())
            )
            .is_err()
        );
        assert_eq!(original_name("../../test.xlsx"), Some("test.xlsx"));
        assert_eq!(original_name("folder\\payload.zip"), Some("payload.zip"));
    }
}
