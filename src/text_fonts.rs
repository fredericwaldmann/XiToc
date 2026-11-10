//! Platform font resources. No font bytes or OS handles enter semantic content.
use crate::text::TextFamily;
use std::sync::Arc;
pub fn families() -> Vec<(String, TextFamily)> {
    let mut out: Vec<(String, TextFamily)> = vec![
        ("DejaVu Sans".into(), TextFamily::Sans),
        ("DejaVu Sans Mono".into(), TextFamily::Monospace),
    ];
    #[cfg(not(target_arch = "wasm32"))]
    out.extend(
        system()
            .names
            .iter()
            .cloned()
            .map(|n| (n.clone(), TextFamily::Installed(n))),
    );
    out.sort_by_key(|(n, _)| n.to_lowercase());
    out.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    out
}
pub fn label(f: &TextFamily) -> &str {
    match f {
        TextFamily::Sans => "DejaVu Sans",
        TextFamily::Monospace => "DejaVu Sans Mono",
        TextFamily::Installed(n) => n,
    }
}
pub fn providers(defs: &egui::FontDefinitions) -> Vec<Arc<dyn egui::epaint::text::FontProvider>> {
    vec![Arc::new(Provider {
        fallback: defs.clone(),
    })]
}
struct Provider {
    fallback: egui::FontDefinitions,
}
impl egui::epaint::text::FontProvider for Provider {
    fn fonts_for_family(&self, family: &egui::FontFamily) -> Vec<egui::FontInsert> {
        let egui::FontFamily::Name(name) = family else {
            return Vec::new();
        };
        let Some(spec) = name.strip_prefix("system:") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut parts = spec.rsplitn(3, ':');
            let italic = parts.next() == Some("true");
            let bold = parts.next() == Some("true");
            if let Some(name) = parts.next() {
                let sys = system();
                let id = sys.db.query(&fontdb::Query {
                    families: &[fontdb::Family::Name(name)],
                    weight: if bold {
                        fontdb::Weight::BOLD
                    } else {
                        fontdb::Weight::NORMAL
                    },
                    style: if italic {
                        fontdb::Style::Italic
                    } else {
                        fontdb::Style::Normal
                    },
                    ..Default::default()
                });
                if let Some(id) = id {
                    let mut cache = sys.cache.lock().unwrap();
                    if let Some(data) = cache
                        .entry(id)
                        .or_insert_with(|| {
                            sys.db.with_face_data(id, |bytes, index| {
                                let mut data = egui::FontData::from_owned(bytes.to_vec());
                                data.index = index;
                                data
                            })
                        })
                        .clone()
                    {
                        out.push(egui::FontInsert::new(
                            &format!("system:{id}"),
                            data,
                            Vec::new(),
                        ));
                    }
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = spec;
        for name in self.fallback.families[&egui::FontFamily::Name("sans".into())].iter() {
            out.push(egui::FontInsert::new(
                name,
                (*self.fallback.font_data[name]).clone(),
                Vec::new(),
            ));
        }
        out
    }
}
#[cfg(not(target_arch = "wasm32"))]
struct System {
    db: fontdb::Database,
    names: Vec<String>,
    cache: std::sync::Mutex<std::collections::HashMap<fontdb::ID, Option<egui::FontData>>>,
}
#[cfg(not(target_arch = "wasm32"))]
fn system() -> &'static System {
    static SYSTEM: std::sync::OnceLock<System> = std::sync::OnceLock::new();
    SYSTEM.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut names: Vec<_> = db
            .faces()
            .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        System {
            db,
            names,
            cache: Default::default(),
        }
    })
}
