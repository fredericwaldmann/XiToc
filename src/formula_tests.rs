use crate::{
    document::{DocumentData, DocumentKind},
    drawing::{Drawing, Surface},
    formula::{FormulaLayout, FormulaObject},
    object::{ObjectSnapshot, Placement},
    object_clipboard::ObjectBundle,
};
use std::collections::HashSet;

fn formula(doc: &DocumentData, x: f64, source: &str) -> FormulaObject {
    let mut formula = FormulaObject::new(
        doc.id(),
        Surface::Canvas,
        [x, 40.0],
        source.into(),
        [20, 50, 100, 255],
    );
    formula.layout = FormulaLayout {
        width: 80.0,
        height: 40.0,
        baseline: 28.0,
    };
    formula
}

#[test]
fn formula_source_edit_is_one_stable_identity_undo_command_and_round_trips_clipboard() {
    let doc = DocumentData::blank(DocumentKind::Whiteboard);
    let mut drawing = Drawing::new(&doc);
    let object = formula(&doc, 20.0, r"\frac{x_1}{\sqrt{2}}");
    let id = object.id;
    let original_source = object.latex_source.clone();
    assert!(drawing.add_formula(object));
    let mut changed = match drawing.snapshot(id).unwrap() {
        ObjectSnapshot::Formula(value) => (**value).clone(),
        _ => unreachable!(),
    };
    changed.latex_source = r"\sum_{i=1}^{n} i^2".into();
    changed.layout = FormulaLayout {
        width: 110.0,
        height: 48.0,
        baseline: 35.0,
    };
    assert!(drawing.replace_formulas(vec![changed], false));
    assert_eq!(drawing.snapshot(id).unwrap().id(), id);
    assert!(drawing.undo().contains(&id));
    let ObjectSnapshot::Formula(restored) = drawing.snapshot(id).unwrap() else {
        panic!()
    };
    assert_eq!(restored.latex_source, original_source);
    assert!(drawing.redo().contains(&id));
    let bundle =
        ObjectBundle::capture_objects([drawing.snapshot(id).unwrap().clone()].into_iter()).unwrap();
    let pasted = bundle.paste(&mut drawing, Surface::Canvas, [300.0, 200.0]);
    assert_eq!(pasted.len(), 1);
    assert_ne!(pasted[0], id);
    let ObjectSnapshot::Formula(copy) = drawing.snapshot(pasted[0]).unwrap() else {
        panic!()
    };
    assert_eq!(copy.latex_source, r"\sum_{i=1}^{n} i^2");
}

#[test]
fn formulas_group_transform_lock_and_keep_notation_unmirrored() {
    let doc = DocumentData::blank(DocumentKind::Whiteboard);
    let mut drawing = Drawing::new(&doc);
    let a = formula(&doc, 30.0, r"E=mc^2");
    let b = formula(&doc, 150.0, r"\alpha_1+\beta^2");
    let (a_id, b_id) = (a.id, b.id);
    assert!(drawing.add_formula(a));
    assert!(drawing.add_formula(b));
    let group = drawing
        .group_selection(&HashSet::from([a_id, b_id]))
        .unwrap();
    assert!(drawing.transform_objects(
        &HashSet::from([group]),
        Placement {
            origin: [90.0, 60.0],
            position: [200.0, 150.0],
            scale: 1.5,
            rotation: 0.4,
            ..Default::default()
        }
    ));
    for id in [a_id, b_id] {
        let ObjectSnapshot::Formula(after) = drawing.snapshot(id).unwrap() else {
            panic!()
        };
        assert_eq!(
            after.latex_source,
            if id == a_id {
                r"E=mc^2"
            } else {
                r"\alpha_1+\beta^2"
            }
        );
        assert!(after.placement.scale > 1.0);
    }
    assert!(drawing.lock_objects(&HashSet::from([group]), true));
    assert!(drawing.is_locked(a_id));
    assert!(!drawing.transform_objects(
        &HashSet::from([group]),
        Placement {
            position: [300.0, 250.0],
            origin: [200.0, 150.0],
            ..Default::default()
        }
    ));
}

#[test]
fn repeated_formula_color_samples_merge_into_one_history_command() {
    let doc = DocumentData::blank(DocumentKind::Whiteboard);
    let mut drawing = Drawing::new(&doc);
    let formula = formula(&doc, 10.0, "x+y");
    let id = formula.id;
    assert!(drawing.add_formula(formula));
    for (index, color) in [[200, 10, 20, 255], [10, 200, 20, 255], [10, 20, 200, 255]]
        .into_iter()
        .enumerate()
    {
        let ObjectSnapshot::Formula(old) = drawing.snapshot(id).unwrap() else {
            panic!()
        };
        let mut next = (**old).clone();
        next.color = color;
        assert!(drawing.replace_formulas(vec![next], index > 0));
    }
    assert!(drawing.undo().contains(&id));
    let ObjectSnapshot::Formula(restored) = drawing.snapshot(id).unwrap() else {
        panic!()
    };
    assert_eq!(restored.color, [20, 50, 100, 255]);
}

#[test]
fn formula_geometry_respects_page_ownership_and_rejects_bad_bounds_or_source_size() {
    let page = DocumentData::blank(DocumentKind::Page);
    let mut drawing = Drawing::new(&page);
    let mut valid = FormulaObject::new(
        page.id(),
        Surface::Page(0),
        [20.0, 20.0],
        r"x^2".into(),
        [0, 0, 0, 255],
    );
    valid.layout = FormulaLayout {
        width: 40.0,
        height: 28.0,
        baseline: 20.0,
    };
    assert!(drawing.add_formula(valid.clone()));
    valid.id = uuid::Uuid::new_v4();
    valid.latex_source = "x".repeat(crate::formula::MAX_FORMULA_BYTES + 1);
    assert!(!drawing.add_formula(valid));
}
