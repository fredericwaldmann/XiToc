#[cfg(target_arch = "wasm32")]
use crate::document::pdf_filename;
use crate::{
    document::DocumentData,
    editor::Editor,
    storage::{Folder, Loaded},
    ui::{
        self, Action,
        dialog::{Creation, Outcome},
        style,
    },
};

struct Opened {
    loaded: Loaded,
    editor: Editor,
}
enum Pending {
    Close(usize),
    #[cfg(not(target_arch = "wasm32"))]
    Workspace,
    Quit,
}

pub struct XiTocApp {
    tree: Folder,
    documents: Vec<Opened>,
    active: Option<usize>,
    selected_folder: String,
    creation: Option<Creation>,
    status: String,
    chrome: ui::chrome::Chrome,
    tabs: ui::tabs::Tabs,
    pending: Option<Pending>,
    quit_approved: bool,
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
            chrome: ui::chrome::Chrome::default(),
            tabs: ui::tabs::Tabs::default(),
            pending: None,
            quit_approved: false,
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
        if ui.input(|i| i.viewport().close_requested()) && self.has_unsaved() && !self.quit_approved
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        if self.pending.is_some() {
            ui.disable();
        }
        if !ui.ctx().text_edit_focused()
            && ui.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                ))
            })
        {
            self.status =
                "Drawing save is unavailable until M2. Unsaved ink remains in memory only.".into();
        }
        self.chrome.theme.apply(ui.ctx());
        ui.set_style(ui.ctx().global_style());
        self.chrome.top(ui);
        let context = self
            .active
            .map(|index| self.documents[index].loaded.document.data.kind().label())
            .unwrap_or("No document");
        let status = if self.has_unsaved() {
            format!(
                "Unsaved drawing · memory only · PDF saving arrives in M2. {}",
                self.status
            )
        } else {
            self.status.clone()
        };
        self.chrome.bottom(ui, &status, context);
        let mut action = None;
        self.show_explorer(ui, &mut action);
        self.show_document_area(ui, &mut action);
        if let Some(action) = action {
            self.act(action, ui.ctx());
        }
        self.creation_dialog(ui.ctx());
        self.discard_dialog(ui.ctx());
        #[cfg(target_arch = "wasm32")]
        {
            // beforeunload in index.html consumes this flag. Browsers may suppress
            // the prompt without prior user interaction or during forced termination.
            if let Some(window) = web_sys::window() {
                let _ = js_sys::Reflect::set(
                    window.as_ref(),
                    &"xitocHasUnsavedChanges".into(),
                    &self.has_unsaved().into(),
                );
            }
        }
    }

    fn has_unsaved(&self) -> bool {
        self.documents
            .iter()
            .any(|opened| opened.editor.canvas.dirty())
    }

    fn discard_dialog(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let mut discard = false;
        let mut cancel = false;
        let scope = if matches!(self.pending, Some(Pending::Close(_))) {
            "This closes the tab and discards its unsaved drawing."
        } else {
            "This discards unsaved drawings in all open tabs."
        };
        egui::Modal::new(egui::Id::unique("discard-unsaved-ink")).show(ctx, |ui| {
            ui.set_max_width((ctx.content_rect().width() - 48.0).clamp(180.0, 360.0));
            ui.heading("Discard unsaved drawing?");
            ui.label(scope);
            ui.label("Ink is only held in memory. PDF saving is not available until M2. Discarding cannot be undone.");
            ui.horizontal_wrapped(|ui| {
                cancel = ui.button("Keep editing").clicked();
                discard = ui.button("Discard drawing").clicked();
            });
            cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
        if cancel {
            self.pending = None;
        } else if discard {
            match self.pending.take().unwrap() {
                Pending::Close(index) => self.close_now(index),
                #[cfg(not(target_arch = "wasm32"))]
                Pending::Workspace => self.choose_workspace(),
                Pending::Quit => {
                    self.quit_approved = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn show_explorer(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        if self.chrome.explorer_hidden {
            return;
        }
        let max_width = (ui.available_width() * 0.45).clamp(132.0, 360.0);
        let can_create = self.can_create();
        let active_path = self
            .active
            .map(|index| self.documents[index].loaded.document.location.name());
        egui::Panel::left("explorer")
            .resizable(true)
            .default_size(248.0)
            .size_range(132.0..=max_width)
            .frame(style::frame(style::palette(ui).sidebar, 10, 10))
            .show(ui, |ui| {
                ui::explorer::Explorer {
                    tree: &self.tree,
                    selected_folder: &mut self.selected_folder,
                    active_path,
                    can_create,
                    status: &mut self.status,
                }
                .show(ui, action);
            });
    }

    fn show_document_area(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = style::palette(ui);
        egui::CentralPanel::default()
            .frame(style::frame(p.workspace, 0, 0))
            .show(ui, |ui| {
                let mut tab_action = None;
                egui::Panel::top("document-tabs")
                    .resizable(false)
                    .exact_size(style::TAB)
                    .frame(style::frame(p.shell, 4, 2))
                    .show(ui, |ui| {
                        let documents = self.documents.iter().map(|opened| ui::tabs::Tab {
                            path: opened.loaded.document.location.name(),
                            kind: opened.loaded.document.data.kind(),
                            read_only: opened.loaded.document.external,
                            dirty: opened.editor.canvas.dirty(),
                        });
                        tab_action = self.tabs.show(ui, documents, self.active);
                    });
                if let Some(tab_action) = tab_action {
                    match tab_action {
                        ui::tabs::TabAction::Activate(index) => self.activate(index),
                        ui::tabs::TabAction::Close(index) => self.close(index),
                    }
                }
                egui::Panel::top("document-toolbar")
                    .resizable(false)
                    .exact_size(40.0)
                    .frame(style::frame(p.surface, 8, 6))
                    .show(ui, |ui| {
                        let document = self.active.map(|index| {
                            let opened = &mut self.documents[index];
                            (&opened.loaded.document, &mut opened.editor)
                        });
                        ui::toolbar::show(ui, document, action);
                    });
                if let Some(index) = self.active {
                    let opened = &mut self.documents[index];
                    if opened.loaded.document.external {
                        egui::Frame::new().inner_margin(24.0).show(ui, |ui| {
                            ui.heading("Read-only PDF");
                            ui.label("PDF preview is not available yet. The original file is preserved and will not be rewritten.");
                        });
                    } else {
                        opened.editor.show(ui, &opened.loaded.document.data);
                    }
                } else {
                    ui::chrome::empty(ui, self.can_create(), &self.selected_folder, action);
                }
            });
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
            if self.documents[index].loaded.bytes != loaded.bytes
                && self.documents[index].editor.canvas.dirty()
            {
                self.status = "External PDF changed; unsaved drawing retained. Close/discard this tab before reloading.".into();
            } else if self.documents[index].loaded.bytes != loaded.bytes {
                self.documents[index] = Opened {
                    editor: Editor::new(&loaded.document.data),
                    loaded,
                };
            }
            self.activate(index);
            return;
        }
        if let Some(active) = self.active {
            self.documents[active].editor.canvas.finish();
        }
        let editor = Editor::new(&loaded.document.data);
        self.documents.push(Opened { loaded, editor });
        self.active = Some(self.documents.len() - 1);
    }

    fn close(&mut self, index: usize) {
        if self.documents[index].editor.canvas.dirty() {
            self.pending = Some(Pending::Close(index));
        } else {
            self.close_now(index);
        }
    }

    fn activate(&mut self, index: usize) {
        if let Some(active) = self.active {
            self.documents[active].editor.canvas.finish();
        }
        self.active = Some(index);
    }

    fn close_now(&mut self, index: usize) {
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
            Action::Workspace => {
                if self.has_unsaved() {
                    self.pending = Some(Pending::Workspace);
                } else {
                    self.choose_workspace();
                }
            }
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
                    if self.documents[index].editor.canvas.dirty() {
                        self.status = "Unsaved ink cannot be exported until M2. The original PDF is unchanged.".into();
                        return;
                    }
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

    #[cfg(not(target_arch = "wasm32"))]
    fn choose_workspace(&mut self) {
        match crate::storage::Workspace::choose() {
            Ok(Some(workspace)) => match workspace.scan() {
                Ok(tree) => {
                    self.tree = tree;
                    self.workspace = Some(workspace);
                    self.selected_folder.clear();
                    self.creation = None;
                    self.documents.clear();
                    self.active = None;
                    self.status =
                        "Workspace open. PDF files are grouped by embedded document type.".into();
                }
                Err(error) => self.status = error,
            },
            Ok(None) => {}
            Err(error) => self.status = error,
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
                            if opened.editor.canvas.dirty() {
                                if loaded.bytes == opened.loaded.bytes {
                                    opened.loaded.document.location = loaded.document.location;
                                }
                            } else {
                                if loaded.document.data != opened.loaded.document.data {
                                    opened.editor = Editor::new(&loaded.document.data);
                                }
                                opened.loaded = loaded;
                            }
                            if self.active == Some(index) {
                                active = Some(reopened.len());
                            }
                            reopened.push(opened);
                        } else if opened.editor.canvas.dirty() {
                            if self.active == Some(index) {
                                active = Some(reopened.len());
                            }
                            reopened.push(opened);
                        }
                    }
                    self.active = active.or_else(|| (!reopened.is_empty()).then_some(0));
                    self.documents = reopened;
                    self.tree = tree;
                    self.status = "Workspace refreshed. Unsaved tabs are retained even if their source changed or disappeared; unchanged documents follow external moves.".into();
                }
                Err(error) => self.status = error,
            }
        }
    }

    fn creation_dialog(&mut self, ctx: &egui::Context) {
        let Some(creation) = &mut self.creation else {
            return;
        };
        match creation.show(ctx) {
            Outcome::Pending => return,
            Outcome::Cancel => {
                self.creation = None;
                return;
            }
            Outcome::Create => {}
        }
        {
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
                    self.status = "Export dialog closed. Confirm any PDF download in your browser; XiToc cannot verify a permanent disk save.".into();
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_entries<'a>(folder: &'a Folder, entries: &mut Vec<&'a crate::storage::Entry>) {
    entries.extend(&folder.files);
    for child in &folder.folders {
        collect_entries(child, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, DocumentKind, Location};
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
    fn dirty_tab_close_requires_confirmation_and_escape_keeps_ink() {
        let data = DocumentData::blank(DocumentKind::Page);
        let mut app = XiTocApp::default();
        app.open(Loaded {
            bytes: crate::pdf::encode(&data).unwrap().into(),
            document: Document {
                data,
                location: Location::new("draft.pdf".into()),
                external: false,
            },
        });
        let mut stroke = crate::drawing::Stroke::new(
            app.documents[0].loaded.document.data.id(),
            crate::drawing::Surface::Page(0),
            [0, 0, 0, 255],
            2.0,
        );
        stroke.push([30.0, 40.0]);
        app.documents[0].editor.canvas.drawing.add(stroke);
        app.close(0);
        assert!(matches!(app.pending, Some(Pending::Close(0))));
        assert_eq!(app.documents.len(), 1);
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        assert!(app.pending.is_none());
        assert!(app.has_unsaved());
        assert_eq!(app.documents[0].editor.canvas.drawing.strokes().len(), 1);
    }

    #[test]
    fn native_close_request_is_cancelled_while_drawing_is_unsaved() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut app = XiTocApp::default();
        app.open(Loaded {
            bytes: crate::pdf::encode(&data).unwrap().into(),
            document: Document {
                data,
                location: Location::new("unsaved.pdf".into()),
                external: false,
            },
        });
        let mut stroke = crate::drawing::Stroke::new(
            app.documents[0].loaded.document.data.id(),
            crate::drawing::Surface::Canvas,
            [0, 0, 0, 255],
            2.0,
        );
        stroke.push([0.0, 0.0]);
        app.documents[0].editor.canvas.drawing.add(stroke);
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let mut output = egui::Context::default().run_ui(input, |ui| app.show(ui));
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(output.viewport_output.values().any(|viewport| {
            viewport
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        }));
        output.textures_delta.clear();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn refresh_and_open_never_replace_unsaved_ink_after_external_changes() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = crate::storage::Workspace::open(dir.path().into()).unwrap();
        let loaded = workspace
            .create("", "draft", DocumentData::blank(DocumentKind::Page))
            .unwrap();
        let id = loaded.document.data.id();
        let original = loaded.bytes.clone();
        let mut app = XiTocApp {
            workspace: Some(workspace),
            ..Default::default()
        };
        app.open(loaded);
        let mut stroke =
            crate::drawing::Stroke::new(id, crate::drawing::Surface::Page(0), [0, 0, 0, 255], 2.0);
        stroke.push([30.0, 40.0]);
        app.documents[0].editor.canvas.drawing.add(stroke);
        let other = crate::pdf::encode(&DocumentData::blank(DocumentKind::Whiteboard)).unwrap();
        std::fs::write(dir.path().join("draft.pdf"), &other).unwrap();
        let replacement = app.workspace.as_ref().unwrap().load("draft.pdf").unwrap();
        app.open(replacement);
        app.refresh();
        assert_eq!(app.documents[0].loaded.document.data.id(), id);
        assert_eq!(app.documents[0].loaded.bytes, original);
        assert_eq!(app.documents[0].editor.canvas.drawing.strokes().len(), 1);
        std::fs::remove_file(dir.path().join("draft.pdf")).unwrap();
        app.refresh();
        assert_eq!(app.documents.len(), 1);
        assert!(app.has_unsaved());
        app.act(Action::Workspace, &egui::Context::default());
        assert!(matches!(app.pending, Some(Pending::Workspace)));
    }

    #[test]
    fn application_fits_resized_windows_and_scaled_themes() {
        for theme in [style::Theme::Light, style::Theme::Dark] {
            for scale in [1.0, 1.5, 2.0] {
                for width in [320.0, 480.0, 800.0, 1200.0] {
                    let mut app = XiTocApp::default();
                    app.chrome.theme = theme;
                    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
                        let data = DocumentData::blank(kind);
                        app.open(Loaded {
                            bytes: crate::pdf::encode(&data).unwrap().into(),
                            document: Document {
                                data,
                                location: Location::new(format!(
                                    "A long document filename {}.pdf",
                                    kind.label()
                                )),
                                external: false,
                            },
                        });
                    }
                    let ctx = egui::Context::default();
                    ctx.set_zoom_factor(scale);
                    // Exercise both layouts after the theme/scale have settled.
                    for active in [0, 1] {
                        app.active = Some(active);
                        for _ in 0..3 {
                            let mut output = ctx.run_ui(egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 640.0))),
                                ..Default::default()
                            }, |ui| {
                                let viewport = ui.max_rect();
                                app.show(ui);
                                assert!(ui.min_rect().right() <= viewport.right() + 1.0,
                                    "horizontal overflow at {width}/{scale}/{theme:?}: {:?} vs {viewport:?}", ui.min_rect());
                                assert!(ui.min_rect().bottom() <= viewport.bottom() + 1.0);
                            });
                            output.textures_delta.clear();
                        }
                    }
                }
            }
        }
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
