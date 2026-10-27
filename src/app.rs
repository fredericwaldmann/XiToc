use crate::{
    document::{DocumentData, DocumentKind, pdf_filename},
    editor::{Editor, indicator},
    storage::{Folder, Loaded},
};
use egui::{self, Sense};

struct Opened {
    loaded: Loaded,
    editor: Editor,
}
struct Creation {
    kind: DocumentKind,
    name: String,
    folder: String,
    error: Option<String>,
    focus: bool,
}

enum Action {
    #[cfg(not(target_arch = "wasm32"))]
    Workspace,
    #[cfg(not(target_arch = "wasm32"))]
    Refresh,
    New(DocumentKind, String),
    Open(String),
    #[cfg(target_arch = "wasm32")]
    Import,
    #[cfg(target_arch = "wasm32")]
    Export,
}

pub struct XiTocApp {
    tree: Folder,
    documents: Vec<Opened>,
    active: Option<usize>,
    selected_folder: String,
    creation: Option<Creation>,
    status: String,
    #[cfg(not(target_arch = "wasm32"))]
    workspace: Option<crate::storage::Workspace>,
    #[cfg(target_arch = "wasm32")]
    browser_files: Vec<Loaded>,
    #[cfg(target_arch = "wasm32")]
    events: (
        std::sync::mpsc::Sender<crate::storage::web::Event>,
        std::sync::mpsc::Receiver<crate::storage::web::Event>,
    ),
}

impl Default for XiTocApp {
    fn default() -> Self {
        Self {
            tree: Folder::default(),
            documents: Vec::new(),
            active: None,
            selected_folder: String::new(),
            creation: None,
            status: "Create a Page Note or Whiteboard to get started.".into(),
            #[cfg(not(target_arch = "wasm32"))]
            workspace: None,
            #[cfg(target_arch = "wasm32")]
            browser_files: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            events: std::sync::mpsc::channel(),
        }
    }
}

impl eframe::App for XiTocApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

impl XiTocApp {
    fn show(&mut self, ui: &mut egui::Ui) {
        #[cfg(target_arch = "wasm32")]
        self.poll_browser();
        let mut action = None;
        egui::Panel::left("explorer").resizable(true).default_size(250.0).size_range(170.0..=420.0)
            .show(ui, |ui| {
                ui.heading("Explorer");
                #[cfg(not(target_arch = "wasm32"))]
                ui.horizontal(|ui| {
                    if ui.button("Open workspace…").clicked() { action = Some(Action::Workspace); }
                    if ui.add_enabled(self.workspace.is_some(), egui::Button::new("Refresh")).clicked() { action = Some(Action::Refresh); }
                });
                #[cfg(target_arch = "wasm32")]
                {
                    if ui.button("Import PDF…").clicked() { action = Some(Action::Import); }
                    ui.weak("Session files · no folder access. Export PDFs to keep them; reimport after reload.");
                }
                let enabled = self.can_create();
                ui.add_enabled_ui(enabled, |ui| {
                    ui.menu_button("+ New Document", |ui| new_choices(ui, &self.selected_folder, &mut action));
                });
                if !enabled { ui.weak("Open a workspace to create PDF documents."); }
                #[cfg(not(target_arch = "wasm32"))]
                if enabled { ui.weak(format!("New in: {}", if self.selected_folder.is_empty() { "workspace root" } else { &self.selected_folder })); }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
                        egui::CollapsingHeader::new(match kind { DocumentKind::Page => "▤ Pages", DocumentKind::Whiteboard => "◇ Whiteboards" })
                            .default_open(true).show(ui, |ui| {
                                if self.tree.name.is_empty() { ui.weak("No documents"); }
                                else { folder_ui(ui, &self.tree, kind, &mut self.selected_folder, &mut action, enabled); }
                            });
                    }
                    show_problems(ui, &self.tree, &mut self.status);
                });
                let blank = ui.allocate_response(ui.available_size(), Sense::click());
                blank.context_menu(|ui| {
                    ui.add_enabled_ui(enabled, |ui| new_choices(ui, &self.selected_folder, &mut action));
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("XiToc");
                if let Some(active) = self.active {
                    ui.label(self.documents[active].loaded.document.location.name());
                    ui.weak(indicator(self.documents[active].loaded.document.data.kind()));
                }
            });
            let mut close = None;
            ui.horizontal_wrapped(|ui| {
                for (index, opened) in self.documents.iter().enumerate() {
                    let path = opened.loaded.document.location.name();
                    let title = path.rsplit('/').next().unwrap_or(path);
                    if ui.selectable_label(self.active == Some(index), title).on_hover_text(format!("{path} · {} bytes", opened.loaded.bytes.len())).clicked() { self.active = Some(index); }
                    if ui.small_button("×").on_hover_text(format!("Close {title}")).clicked() { close = Some(index); }
                }
            });
            if let Some(index) = close { self.close(index); }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for tool in ["Pen", "Eraser", "Hand"] { ui.add_enabled(false, egui::Button::new(tool)); }
                ui.weak("Drawing tools arrive in M1. Navigation is session-only.");
                #[cfg(target_arch = "wasm32")]
                if ui.add_enabled(self.active.is_some(), egui::Button::new("Export PDF…")).clicked() { action = Some(Action::Export); }
            });
            ui.label(&self.status);
            ui.separator();
            if let Some(active) = self.active {
                let opened = &mut self.documents[active];
                if opened.loaded.document.external {
                    ui.heading(format!("PDF preview unavailable · read-only {}", opened.loaded.document.data.kind().label()));
                    ui.label("PDF content rendering/import is not implemented yet. This file is not shown as a blank note and will not be rewritten.");
                } else { opened.editor.show(ui, &opened.loaded.document.data); }
            } else {
                ui.centered_and_justified(|ui| ui.label("Open or create a document from the Explorer."));
            }
        });
        if let Some(action) = action {
            self.act(action, ui.ctx());
        }
        self.creation_dialog(ui.ctx());
    }

    fn can_create(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.workspace.is_some()
        }
        #[cfg(target_arch = "wasm32")]
        {
            true
        }
    }

    fn open(&mut self, loaded: Loaded) {
        if let Some(index) = self
            .documents
            .iter()
            .position(|d| d.loaded.document.location == loaded.document.location)
        {
            if self.documents[index].loaded.bytes != loaded.bytes {
                self.documents[index] = Opened {
                    editor: Editor::new(&loaded.document.data),
                    loaded,
                };
            }
            self.active = Some(index);
            return;
        }
        let editor = Editor::new(&loaded.document.data);
        self.documents.push(Opened { loaded, editor });
        self.active = Some(self.documents.len() - 1);
    }

    fn close(&mut self, index: usize) {
        self.documents.remove(index);
        self.active = match (self.documents.len(), self.active) {
            (0, _) => None,
            (len, Some(active)) if active == index => Some(index.min(len - 1)),
            (_, Some(active)) if active > index => Some(active - 1),
            (_, active) => active,
        };
    }

    fn act(&mut self, action: Action, _ctx: &egui::Context) {
        match action {
            Action::New(kind, folder) => {
                self.creation = Some(Creation {
                    kind,
                    name: String::new(),
                    folder,
                    error: None,
                    focus: true,
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            Action::Workspace => match crate::storage::Workspace::choose() {
                Ok(Some(workspace)) => match workspace.scan() {
                    Ok(tree) => {
                        self.tree = tree;
                        self.workspace = Some(workspace);
                        self.selected_folder.clear();
                        self.creation = None;
                        self.documents.clear();
                        self.active = None;
                        self.status =
                            "Workspace open. PDF files are grouped by embedded document type."
                                .into();
                    }
                    Err(error) => self.status = error,
                },
                Ok(None) => {}
                Err(error) => self.status = error,
            },
            #[cfg(not(target_arch = "wasm32"))]
            Action::Refresh => self.refresh(),
            Action::Open(path) => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(workspace) = &self.workspace {
                    match workspace.load(&path) {
                        Ok(loaded) => {
                            self.status = format!("Opened {path}");
                            self.open(loaded);
                        }
                        Err(error) => self.status = error,
                    }
                }
                #[cfg(target_arch = "wasm32")]
                if let Some(loaded) = self
                    .browser_files
                    .iter()
                    .find(|f| f.document.location.name() == path)
                    .cloned()
                {
                    self.open(loaded);
                }
            }

            #[cfg(target_arch = "wasm32")]
            Action::Import => crate::storage::web::import(self.events.0.clone(), _ctx.clone()),
            #[cfg(target_arch = "wasm32")]
            Action::Export => {
                if let Some(index) = self.active {
                    let loaded = &self.documents[index].loaded;
                    crate::storage::web::export(
                        loaded.document.location.name().into(),
                        loaded.bytes.clone(),
                        self.events.0.clone(),
                        _ctx.clone(),
                    );
                }
            }
        }
    }

    fn refresh(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(workspace) = &self.workspace {
            match workspace.scan() {
                Ok(tree) => {
                    let mut entries = Vec::new();
                    collect_entries(&tree, &mut entries);
                    let mut active = None;
                    let mut reopened = Vec::new();
                    for (index, mut opened) in
                        std::mem::take(&mut self.documents).into_iter().enumerate()
                    {
                        let original = opened.loaded.document.location.name();
                        let matches: Vec<_> = entries
                            .iter()
                            .filter(|entry| {
                                entry.document_id == Some(opened.loaded.document.data.id())
                            })
                            .collect();
                        let path = if entries
                            .iter()
                            .any(|entry| entry.path == original && entry.kind.is_some())
                        {
                            Some(original.to_owned())
                        } else if matches.len() == 1 {
                            Some(matches[0].path.clone())
                        } else {
                            None
                        };
                        if let Some(path) = path
                            && let Ok(loaded) = workspace.load(&path)
                        {
                            if loaded.document.data != opened.loaded.document.data {
                                opened.editor = Editor::new(&loaded.document.data);
                            }
                            opened.loaded = loaded;
                            if self.active == Some(index) {
                                active = Some(reopened.len());
                            }
                            reopened.push(opened);
                        }
                    }
                    self.active = active.or_else(|| (!reopened.is_empty()).then_some(0));
                    self.documents = reopened;
                    self.tree = tree;
                    self.status = "Workspace refreshed. Moved native documents retain their identity; missing/unreadable tabs are closed.".into();
                }
                Err(error) => self.status = error,
            }
        }
    }

    fn creation_dialog(&mut self, ctx: &egui::Context) {
        let Some(creation) = &mut self.creation else {
            return;
        };
        let mut create = false;
        let mut cancel = false;
        egui::Window::new(format!("New {}", creation.kind.label()))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                ui.label(format!(
                    "Folder: {}",
                    if creation.folder.is_empty() {
                        "workspace root"
                    } else {
                        &creation.folder
                    }
                ));
                #[cfg(target_arch = "wasm32")]
                ui.label(
                    "Browser session · export the PDF to choose where to keep it after reload.",
                );
                ui.label("Filename");
                let field = ui.text_edit_singleline(&mut creation.name);
                if creation.focus {
                    field.request_focus();
                    creation.focus = false;
                }
                if field.changed() {
                    creation.error = None;
                }
                let valid = pdf_filename(&creation.name);
                if let Err(error) = &valid {
                    ui.weak(error);
                }
                if let Some(error) = &creation.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                create = valid.is_ok()
                    && field.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                ui.horizontal(|ui| {
                    create |= ui
                        .add_enabled(
                            valid.is_ok(),
                            egui::Button::new(if cfg!(target_arch = "wasm32") {
                                "Create & export PDF"
                            } else {
                                "Create"
                            }),
                        )
                        .clicked();
                    cancel |= ui.button("Cancel").clicked();
                });
            });
        if cancel {
            self.creation = None;
            return;
        }
        if create {
            let creation = self.creation.as_ref().unwrap();
            let data = DocumentData::blank(creation.kind);
            #[cfg(not(target_arch = "wasm32"))]
            let result = self
                .workspace
                .as_ref()
                .ok_or("Open a workspace first".to_owned())
                .and_then(|workspace| workspace.create(&creation.folder, &creation.name, data));
            #[cfg(target_arch = "wasm32")]
            let result = pdf_filename(&creation.name).and_then(|name| {
                if !creation.folder.is_empty() {
                    return Err("Browser sessions do not expose disk folders".into());
                }
                if self.tree.files.iter().any(|f| f.path == name) {
                    return Err("A session document with this name already exists".into());
                }
                crate::pdf::encode(&data).map(|bytes| Loaded {
                    document: crate::document::Document {
                        data,
                        location: crate::document::Location::new(name),
                        external: false,
                    },
                    bytes: bytes.into(),
                })
            });
            match result {
                Ok(loaded) => {
                    self.status = format!("Created {}", loaded.document.location.name());
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.tree.name = "Session PDFs".into();
                        self.tree.files.push(crate::storage::Entry {
                            path: loaded.document.location.name().into(),
                            kind: Some(loaded.document.data.kind()),
                            document_id: Some(loaded.document.data.id()),
                            problem: None,
                        });
                        self.status
                            .push_str(" · session only; export the PDF to keep it");
                    }
                    #[cfg(target_arch = "wasm32")]
                    self.browser_files.push(loaded.clone());
                    self.open(loaded);
                    self.refresh();
                    self.creation = None;
                    #[cfg(target_arch = "wasm32")]
                    self.act(Action::Export, ctx);
                }
                Err(error) => {
                    if let Some(creation) = &mut self.creation {
                        creation.error = Some(error.clone());
                    }
                    self.status = error;
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_browser(&mut self) {
        use crate::storage::web::Event;
        while let Ok(event) = self.events.1.try_recv() {
            match event {
                Event::Imported(Ok(loaded)) => {
                    let path = loaded.document.location.name().to_owned();
                    if self.tree.files.iter().any(|f| f.path == path) {
                        self.status = "That filename is already in this session; reopen it from Explorer or import a distinct filename.".into();
                        continue;
                    }
                    self.tree.name = "Session PDFs".into();
                    self.tree.files.push(crate::storage::Entry {
                        path: path.clone(),
                        kind: Some(loaded.document.data.kind()),
                        document_id: Some(loaded.document.data.id()),
                        problem: None,
                    });
                    self.status = format!("Imported {path} for this session");
                    self.browser_files.push(loaded.clone());
                    self.open(loaded);
                }
                Event::Imported(Err(error)) | Event::Export(Err(error)) => self.status = error,
                Event::Export(Ok(())) => {
                    self.status = "Export requested. Confirm the browser download; XiToc cannot verify a permanent disk save.".into();
                }
            }
        }
    }
}

fn new_choices(ui: &mut egui::Ui, folder: &str, action: &mut Option<Action>) {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        if ui.button(format!("New {}", kind.label())).clicked() {
            *action = Some(Action::New(kind, folder.into()));
            ui.close();
        }
    }
}

fn folder_ui(
    ui: &mut egui::Ui,
    folder: &Folder,
    kind: DocumentKind,
    selected: &mut String,
    action: &mut Option<Action>,
    can_create: bool,
) {
    let response = egui::CollapsingHeader::new(format!("▸ {}", folder.name))
        .id_salt((kind.label(), &folder.path))
        .default_open(true)
        .show(ui, |ui| {
            for child in &folder.folders {
                folder_ui(ui, child, kind, selected, action, can_create);
            }
            for file in folder.files.iter().filter(|f| f.kind == Some(kind)) {
                let title = file.path.rsplit('/').next().unwrap_or(&file.path);
                let response = ui
                    .push_id((&file.path, file.document_id), |ui| {
                        ui.selectable_label(
                            false,
                            format!(
                                "{} {title}",
                                if kind == DocumentKind::Page {
                                    "▤"
                                } else {
                                    "◇"
                                }
                            ),
                        )
                        .on_hover_text(&file.path)
                    })
                    .inner;
                if response.clicked() {
                    *selected = folder.path.clone();
                    *action = Some(Action::Open(file.path.clone()));
                }
                response.context_menu(|ui| {
                    ui.add_enabled_ui(can_create, |ui| new_choices(ui, &folder.path, action));
                });
            }
        });
    if response.header_response.clicked() {
        *selected = folder.path.clone();
    }
    response.header_response.context_menu(|ui| {
        *selected = folder.path.clone();
        ui.label(if folder.path.is_empty() {
            if cfg!(target_arch = "wasm32") {
                "Browser session"
            } else {
                "Workspace root"
            }
        } else {
            &folder.path
        });
        ui.add_enabled_ui(can_create, |ui| new_choices(ui, &folder.path, action));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_entries<'a>(folder: &'a Folder, entries: &mut Vec<&'a crate::storage::Entry>) {
    entries.extend(&folder.files);
    for child in &folder.folders {
        collect_entries(child, entries);
    }
}

fn show_problems(ui: &mut egui::Ui, folder: &Folder, status: &mut String) {
    for file in folder.files.iter().filter(|f| f.problem.is_some()) {
        if ui
            .button(format!("⚠ {}", file.path))
            .on_hover_text("Unreadable PDF; original unchanged")
            .clicked()
        {
            *status = file.problem.clone().unwrap_or_default();
        }
    }
    for child in &folder.folders {
        show_problems(ui, child, status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Location};
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn refresh_tracks_a_real_move_and_closes_deleted_file_without_changing_kind() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = crate::storage::Workspace::open(dir.path().into()).unwrap();
        let loaded = workspace
            .create(
                "",
                "original",
                DocumentData::blank(DocumentKind::Whiteboard),
            )
            .unwrap();
        let id = loaded.document.data.id();
        let mut app = XiTocApp {
            workspace: Some(workspace),
            ..Default::default()
        };
        app.open(loaded);
        std::fs::rename(
            dir.path().join("original.pdf"),
            dir.path().join("renamed.pdf"),
        )
        .unwrap();
        app.refresh();
        assert_eq!(app.documents.len(), 1);
        assert_eq!(
            app.documents[0].loaded.document.location.name(),
            "renamed.pdf"
        );
        assert_eq!(
            app.documents[0].loaded.document.data.kind(),
            DocumentKind::Whiteboard
        );
        assert_eq!(app.documents[0].loaded.document.data.id(), id);
        std::fs::remove_file(dir.path().join("renamed.pdf")).unwrap();
        app.refresh();
        assert!(app.documents.is_empty());
        assert_eq!(app.active, None);
    }

    #[test]
    fn opening_and_closing_documents_uses_metadata_without_a_mode_switch() {
        let mut app = XiTocApp::default();
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let data = DocumentData::blank(kind);
            app.open(Loaded {
                bytes: crate::pdf::encode(&data).unwrap().into(),
                document: Document {
                    data,
                    location: Location::new(format!("{}.pdf", kind.label())),
                    external: false,
                },
            });
            assert_eq!(
                app.documents[app.active.unwrap()]
                    .loaded
                    .document
                    .data
                    .kind(),
                kind
            );
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show(ui));
            output.textures_delta.clear();
            assert_eq!(
                app.documents[app.active.unwrap()]
                    .loaded
                    .document
                    .data
                    .kind(),
                kind
            );
        }
        app.close(0);
        assert_eq!(app.active, Some(0));
        assert_eq!(
            app.documents[0].loaded.document.data.kind(),
            DocumentKind::Whiteboard
        );
    }
}
