//! In-memory editable objects and reversible commands. No UI, IO or PDF dependencies.
use crate::document::{DocumentData, Layout};
use crate::object::{CanvasObject, ObjectSnapshot};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

pub const POINTS_PER_MM: f64 = 72.0 / 25.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Surface {
    Page(usize),
    Canvas,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct InkPoint {
    pub position: [f64; 2],
    /// Normalized hardware measurement; None means unavailable (including mouse).
    #[serde(default)]
    pub pressure: Option<f32>,
}
impl InkPoint {
    pub fn mouse(position: [f64; 2]) -> Self {
        Self {
            position,
            pressure: None,
        }
    }
}

pub fn normalize_pressure(value: Option<f32>) -> Option<f32> {
    value.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0))
}
pub fn mix_pressure(a: Option<f32>, b: Option<f32>, t: f32) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a + (b - a) * t),
        _ => None,
    }
}

/// Captured at pointer-down; old strokes keep their constant-width appearance.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PressureStyle {
    pub enabled: bool,
    pub strength: u8,
    pub minimum: u8,
}
impl Default for PressureStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            strength: 100,
            minimum: 20,
        }
    }
}
impl PressureStyle {
    pub fn factor(self, pressure: Option<f32>) -> f64 {
        if !self.enabled {
            return 1.0;
        }
        let Some(p) = pressure else {
            return 1.0;
        };
        let minimum = f64::from(self.minimum) / 100.0;
        let response = minimum + (1.0 - minimum) * f64::from(p);
        1.0 + (response - 1.0) * f64::from(self.strength) / 100.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EraserMode {
    #[default]
    Stroke,
    Partial,
}
impl EraserMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stroke => "Stroke Eraser",
            Self::Partial => "Partial Eraser",
        }
    }
}

/// Versioned per-stroke processing; missing metadata retains the original M1 path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Smoothing {
    #[default]
    Legacy,
    /// Explicit surviving centerline after clipping; never smooth it again.
    PolylineV1,
    AdaptiveV1 {
        strength: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub placement: crate::object::Placement,
    #[serde(default)]
    pub pen_type: crate::pen::PenType,
    #[serde(default)]
    pub pressure_style: PressureStyle,
    pub id: Uuid,
    pub document_id: Uuid,
    pub surface: Surface,
    pub points: Vec<InkPoint>,
    pub color: [u8; 4],
    /// PDF points (1/72 inch), independent of display pixels and zoom.
    pub width: f64,
    #[serde(default)]
    pub smoothing: Smoothing,
}
impl Stroke {
    pub fn new(document_id: Uuid, surface: Surface, color: [u8; 4], width: f64) -> Self {
        Self {
            placement: Default::default(),
            pen_type: crate::pen::PenType::Monoline,
            pressure_style: PressureStyle::default(),
            locked: false,
            id: Uuid::new_v4(),
            document_id,
            surface,
            points: Vec::new(),
            color,
            width,
            smoothing: Smoothing::Legacy,
        }
    }
    pub fn path(&self) -> Vec<[f64; 2]> {
        let mut path = match self.smoothing {
            Smoothing::Legacy => smooth(&self.points),
            Smoothing::PolylineV1 => self.points.iter().map(|p| p.position).collect(),
            Smoothing::AdaptiveV1 { strength } => crate::smoothing::path(&self.points, strength),
        };
        if !self.placement.is_identity() {
            for p in &mut path {
                *p = self.placement.apply(*p);
            }
        }
        path
    }
    pub fn visible_width(&self) -> f64 {
        self.width * self.placement.scale
    }
    pub fn push_pressure(&mut self, position: [f64; 2], pressure: Option<f32>) {
        let pressure = normalize_pressure(pressure);
        if pressure.is_none() {
            self.push(position);
            return;
        }
        let point = InkPoint { position, pressure };
        if self.points.last() != Some(&point) {
            self.points.push(point);
        }
    }
    pub fn push(&mut self, position: [f64; 2]) {
        let point = InkPoint::mouse(position);
        if self.points.last() != Some(&point) {
            self.points.push(point);
        }
    }
}

/// Screen = origin + (world - center) * scale. Subtract in f64 before conversion
/// at the UI boundary, including far-from-origin Whiteboard coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub origin: [f64; 2],
    pub center: [f64; 2],
    pub scale: f64,
}
impl Transform {
    pub fn to_screen(self, point: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|i| self.origin[i] + (point[i] - self.center[i]) * self.scale)
    }
    pub fn to_document(self, point: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|i| self.center[i] + (point[i] - self.origin[i]) / self.scale)
    }
}

// Each command retains just the affected objects and their original order.
// New object commands can extend this enum with their inverse state.
#[derive(Debug)]
enum Command {
    Erase(Vec<(usize, ObjectSnapshot)>),
    Replace {
        before: Vec<(usize, ObjectSnapshot)>,
        after: Vec<(usize, ObjectSnapshot)>,
    },
}
fn remove_at(strokes: &mut Vec<ObjectSnapshot>, removed: &[(usize, ObjectSnapshot)]) {
    if removed.is_empty() {
        return;
    }
    let mut removed = removed.iter().peekable();
    let mut index = 0;
    strokes.retain(|_| {
        let keep = removed.peek().is_none_or(|(target, _)| *target != index);
        if !keep {
            removed.next();
        }
        index += 1;
        keep
    });
}
fn insert_at(strokes: &mut Vec<ObjectSnapshot>, removed: &[(usize, ObjectSnapshot)]) {
    if removed.is_empty() {
        return;
    }
    // Creation/batch append is O(new objects), preserving the ink fast path.
    if removed
        .iter()
        .enumerate()
        .all(|(i, (index, _))| *index == strokes.len() + i)
    {
        strokes.extend(removed.iter().map(|(_, object)| object.clone()));
        return;
    }
    // Merge backwards in linear time instead of shifting the tail
    // once per restored stroke. All slots contain valid Arcs.
    let original_len = strokes.len();
    strokes.extend(removed.iter().map(|(_, stroke)| stroke.clone()));
    let mut source = original_len;
    let mut removed = removed.iter().rev().peekable();
    for target in (0..strokes.len()).rev() {
        if removed.peek().is_some_and(|(index, _)| *index == target) {
            strokes[target] = removed.next().unwrap().1.clone();
        } else {
            source -= 1;
            strokes[target] = strokes[source].clone();
        }
    }
}
impl Command {
    fn apply(&self, strokes: &mut Vec<ObjectSnapshot>) {
        match self {
            Self::Erase(removed) => remove_at(strokes, removed),
            Self::Replace { before, after } => {
                remove_at(strokes, before);
                insert_at(strokes, after);
            }
        }
    }
    fn revert(&self, strokes: &mut Vec<ObjectSnapshot>) {
        match self {
            Self::Erase(removed) => insert_at(strokes, removed),
            Self::Replace { before, after } => {
                remove_at(strokes, after);
                insert_at(strokes, before);
            }
        }
    }
}

struct HistoryEntry {
    command: Command,
    // UI selection intent excludes automatically updated dependent connectors.
    selection: Option<Vec<Uuid>>,
    undo_selection: Option<Vec<Uuid>>,
}

pub struct Drawing {
    owner: Uuid,
    layout: Layout,
    items: Vec<ObjectSnapshot>,
    parents: std::collections::HashMap<Uuid, Uuid>,
    pub editing_group: Option<Uuid>,
    live_ids: std::collections::HashSet<Uuid>,
    object_slots: std::collections::HashMap<Uuid, usize>,
    // Transient Schmitt state for live target previews; never canonical content.
    routing_hints: std::collections::HashMap<Uuid, [Option<(Uuid, crate::connector::Port)>; 2]>,
    pub(crate) links: std::collections::HashMap<Uuid, std::collections::HashSet<Uuid>>,
    // Derived ink-only view: Arc references, never a second canonical store.
    strokes: Vec<Arc<Stroke>>,
    history: Vec<HistoryEntry>,
    cursor: usize,
    revision: u64,
}
impl Drawing {
    pub fn new(data: &DocumentData) -> Self {
        Self {
            owner: data.id(),
            layout: data.layout().clone(),
            items: Vec::new(),
            parents: Default::default(),
            editing_group: None,
            live_ids: Default::default(),
            object_slots: Default::default(),
            routing_hints: Default::default(),
            links: Default::default(),
            strokes: Vec::new(),
            history: Vec::new(),
            cursor: 0,
            revision: 0,
        }
    }
    /// Changes only when committed objects change; invalidates derived spatial data.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn owner(&self) -> Uuid {
        self.owner
    }
    pub fn strokes(&self) -> &[Arc<Stroke>] {
        &self.strokes
    }
    pub fn items(&self) -> &[ObjectSnapshot] {
        &self.items
    }
    pub fn objects(&self) -> impl DoubleEndedIterator<Item = &dyn CanvasObject> {
        self.items.iter().map(ObjectSnapshot::as_object)
    }
    pub fn object(&self, id: crate::object::ObjectId) -> Option<&dyn CanvasObject> {
        self.snapshot(id).map(ObjectSnapshot::as_object)
    }
    pub fn snapshot(&self, id: Uuid) -> Option<&ObjectSnapshot> {
        self.object_slots.get(&id).map(|i| &self.items[*i])
    }
    fn index_object(&mut self, index: usize) {
        let object = &self.items[index];
        self.object_slots.insert(object.id(), index);
        if let ObjectSnapshot::Connector(c) = object {
            for end in c.ends() {
                if let Some(target) = end.target() {
                    self.links.entry(target).or_default().insert(c.id);
                }
            }
        }
    }
    fn refresh_ink_view(&mut self) {
        self.parents.clear();
        for o in &self.items {
            if let ObjectSnapshot::Group(g) = o {
                for child in &g.children {
                    self.parents.insert(*child, g.id);
                }
            }
        }
        self.object_slots.clear();
        self.links.clear();
        for index in 0..self.items.len() {
            self.index_object(index);
        }
        if self
            .editing_group
            .is_some_and(|id| !self.items.iter().any(|o| o.id() == id) || self.is_locked(id))
        {
            self.editing_group = None;
        }
        self.live_ids = self.items.iter().map(ObjectSnapshot::id).collect();
        self.strokes = self
            .items
            .iter()
            .filter_map(|o| {
                if let ObjectSnapshot::Stroke(s) = o {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect();
    }
    #[allow(dead_code)]
    pub fn set_z_order(&mut self, id: Uuid, target: usize) -> bool {
        // Legacy absolute-slot API is limited to the flat context. UI uses Arrange.
        if self.is_locked(id)
            || self
                .items
                .iter()
                .any(|o| matches!(o, ObjectSnapshot::Group(_)))
        {
            return false;
        }
        let Some(source) = self.items.iter().position(|o| o.id() == id) else {
            return false;
        };
        if target >= self.items.len() || source == target {
            return false;
        }
        let lo = source.min(target);
        let hi = source.max(target);
        let before: Vec<_> = (lo..=hi).map(|i| (i, self.items[i].clone())).collect();
        let mut ordered: Vec<_> = before.iter().map(|(_, s)| s.clone()).collect();
        let moved = ordered.remove(source - lo);
        ordered.insert(target - lo, moved);
        self.execute(Command::Replace {
            before,
            after: ordered
                .into_iter()
                .enumerate()
                .map(|(i, s)| (lo + i, s))
                .collect(),
        });
        true
    }
    pub fn dirty(&self) -> bool {
        !self.items.is_empty()
    }
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_redo(&self) -> bool {
        self.cursor < self.history.len()
    }
    pub fn bounds(&self, surface: Surface) -> Option<[f64; 2]> {
        match (&self.layout, surface) {
            (Layout::Page { pages }, Surface::Page(index)) => pages.get(index).map(|page| {
                [
                    page.width_mm * POINTS_PER_MM,
                    page.height_mm * POINTS_PER_MM,
                ]
            }),
            _ => None,
        }
    }
    pub fn contains(&self, surface: Surface, position: [f64; 2]) -> bool {
        if !position.iter().all(|v| v.is_finite()) {
            return false;
        }
        match (&self.layout, surface) {
            (Layout::Whiteboard { .. }, Surface::Canvas) => true,
            (Layout::Page { .. }, Surface::Page(_)) => self
                .bounds(surface)
                .is_some_and(|bounds| (0..2).all(|i| (0.0..=bounds[i]).contains(&position[i]))),
            _ => false,
        }
    }
    pub(crate) fn valid_stroke(&self, stroke: &Stroke) -> bool {
        stroke.placement.valid()
            && stroke.document_id() == self.owner
            && !stroke.id.is_nil()
            && !matches!(stroke.smoothing, Smoothing::AdaptiveV1 { strength } if strength > 100)
            && stroke.pressure_style.strength <= 100
            && (1..=100).contains(&stroke.pressure_style.minimum)
            && !stroke.points.is_empty()
            && stroke.width.is_finite()
            && (0.1..=24.0).contains(&stroke.width)
            && stroke.points.iter().all(|p| {
                self.contains(stroke.surface, stroke.placement.apply(p.position))
                    && p.pressure
                        .is_none_or(|p| p.is_finite() && (0.0..=1.0).contains(&p))
            })
    }
    pub fn valid_object(&self, object: &ObjectSnapshot) -> bool {
        match object {
            ObjectSnapshot::Group(g) => g.document_id == self.owner && !g.id.is_nil(),
            ObjectSnapshot::Text(t) => {
                t.valid()
                    && t.document_id == self.owner
                    && t.corners().iter().all(|p| self.contains(t.surface, *p))
            }
            ObjectSnapshot::Connector(c) => self.valid_connector(c, |id| self.snapshot(id)),
            ObjectSnapshot::Stroke(s) => self.valid_stroke(s),
            ObjectSnapshot::Shape(s) => {
                s.valid()
                    && s.document_id == self.owner
                    && s.geometry_bounds()
                        .corners()
                        .iter()
                        .all(|p| self.contains(s.surface, *p))
            }
        }
    }
    pub fn valid_connector<'a>(
        &self,
        c: &crate::connector::Connector,
        lookup: impl Fn(Uuid) -> Option<&'a ObjectSnapshot>,
    ) -> bool {
        c.valid() && c.document_id==self.owner && c.ends().iter().all(|e| {
            self.contains(c.surface,e.position()) && e.target().is_none_or(|id|matches!(lookup(id),Some(ObjectSnapshot::Shape(s)) if s.document_id==self.owner && s.surface==c.surface && !s.kind().segment()))
        })
    }
    pub(crate) fn clear_routing_hints(&mut self) {
        self.routing_hints.clear();
    }
    pub(crate) fn hint_routing(&mut self, c: &crate::connector::Connector) {
        self.routing_hints.insert(
            c.id,
            c.ends().map(|end| match end {
                crate::connector::Endpoint::Attached {
                    object_id,
                    anchor,
                    automatic: true,
                    ..
                } => Some((*object_id, *anchor)),
                _ => None,
            }),
        );
    }
    pub(crate) fn apply_routing_hint(&self, c: &mut crate::connector::Connector) {
        if let Some(hints) = self.routing_hints.get(&c.id) {
            for (end, hint) in c.ends_mut().into_iter().zip(hints) {
                if let (
                    crate::connector::Endpoint::Attached {
                        object_id,
                        anchor,
                        automatic: true,
                        ..
                    },
                    Some((id, port)),
                ) = (end, hint)
                    && object_id == id
                {
                    *anchor = *port;
                }
            }
        }
    }
    fn resolve_connector(
        &self,
        c: &mut crate::connector::Connector,
        replacements: &std::collections::HashMap<Uuid, ObjectSnapshot>,
    ) {
        c.resolve_routes(
            |id| match replacements.get(&id).or_else(|| self.snapshot(id)) {
                Some(ObjectSnapshot::Shape(s)) => Some(s),
                _ => None,
            },
        );
    }
    /// Resolve only reverse-index dependents, within the same target transaction.
    fn update_links(
        &self,
        before: &mut Vec<(usize, ObjectSnapshot)>,
        after: &mut Vec<(usize, ObjectSnapshot)>,
    ) -> bool {
        let replacements: std::collections::HashMap<_, _> =
            after.iter().map(|(_, o)| (o.id(), o.clone())).collect();
        let affected: std::collections::HashSet<_> = after
            .iter()
            .filter(|(_, o)| matches!(o, ObjectSnapshot::Shape(_)))
            .flat_map(|(_, o)| self.links.get(&o.id()).into_iter().flatten().copied())
            .collect();
        let mut before_ids: std::collections::HashSet<_> =
            before.iter().map(|(_, o)| o.id()).collect();
        let mut after_slots: std::collections::HashMap<_, _> = after
            .iter()
            .enumerate()
            .map(|(i, (_, o))| (o.id(), i))
            .collect();
        for id in affected {
            let Some(ObjectSnapshot::Connector(original)) = self.snapshot(id) else {
                continue;
            };
            let mut c = match replacements.get(&id) {
                Some(ObjectSnapshot::Connector(c)) => (**c).clone(),
                _ => (**original).clone(),
            };
            self.resolve_connector(&mut c, &replacements);
            if c == **original && !replacements.contains_key(&id) {
                continue;
            }
            let index = self.object_slots[&id];
            if before_ids.insert(id) {
                before.push((index, self.items[index].clone()));
            }
            if let Some(slot) = after_slots.get(&id) {
                after[*slot].1 = c.into();
            } else {
                after_slots.insert(id, after.len());
                after.push((index, c.into()));
            }
        }
        before.sort_by_key(|(i, _)| *i);
        after.sort_by_key(|(i, _)| *i);
        let resolved: std::collections::HashMap<_, _> =
            after.iter().map(|(_, o)| (o.id(), o)).collect();
        after.iter().all(|(_, o)| match o {
            ObjectSnapshot::Connector(c) => self.valid_connector(c, |id| {
                resolved.get(&id).copied().or_else(|| self.snapshot(id))
            }),
            _ => self.valid_object(o),
        })
    }
    pub fn replace_connector(&mut self, mut c: crate::connector::Connector) -> bool {
        if self.is_locked(c.id) {
            return false;
        }
        let Some(ObjectSnapshot::Connector(old)) = self.snapshot(c.id) else {
            return false;
        };
        if c.document_id != old.document_id || c.surface != old.surface {
            return false;
        }
        self.resolve_connector(&mut c, &Default::default());
        if c == **old || !self.valid_object(&ObjectSnapshot::from(c.clone())) {
            return false;
        }
        let index = self.object_slots[&c.id];
        let before = vec![(index, self.items[index].clone())];
        self.execute(Command::Replace {
            before,
            after: vec![(index, c.into())],
        });
        true
    }
    pub fn edit_connectors(
        &mut self,
        ids: &std::collections::HashSet<Uuid>,
        edit: impl Fn(&mut crate::connector::Connector),
    ) -> bool {
        let mut before = Vec::new();
        let mut after = Vec::new();
        for id in ids {
            if self.is_locked(*id) {
                continue;
            }
            let Some(ObjectSnapshot::Connector(old)) = self.snapshot(*id) else {
                continue;
            };
            let mut c = (**old).clone();
            edit(&mut c);
            self.resolve_connector(&mut c, &Default::default());
            if c == **old {
                continue;
            }
            if !self.valid_connector(&c, |id| self.snapshot(id)) {
                return false;
            }
            let index = self.object_slots[id];
            before.push((index, self.items[index].clone()));
            after.push((index, c.into()));
        }
        if before.is_empty() {
            return false;
        }
        before.sort_by_key(|(i, _)| *i);
        after.sort_by_key(|(i, _)| *i);
        self.execute(Command::Replace { before, after });
        true
    }
    pub fn add_connector(&mut self, mut c: crate::connector::Connector) -> bool {
        self.resolve_connector(&mut c, &Default::default());
        self.add_snapshot(c.into())
    }
    pub fn add(&mut self, stroke: Stroke) -> bool {
        self.add_snapshot(stroke.into())
    }
    pub fn add_shape(&mut self, shape: crate::shape::Shape) -> bool {
        self.add_snapshot(shape.into())
    }
    fn add_snapshot(&mut self, object: ObjectSnapshot) -> bool {
        if !self.valid_object(&object) || self.live_ids.contains(&object.id()) {
            return false;
        }
        let index = self.items.len();
        self.execute(Command::Replace {
            before: Vec::new(),
            after: vec![(index, object)],
        });
        true
    }
    pub fn erase(&mut self, ids: &std::collections::HashSet<Uuid>) {
        let ids: std::collections::HashSet<_> = ids
            .iter()
            .filter(|id| !self.is_locked(**id))
            .copied()
            .collect();
        let ids = &self.expanded(&ids);
        if ids.iter().any(|id| self.is_locked(*id)) {
            return;
        }
        let removed: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, o)| ids.contains(&o.id()))
            .map(|(i, o)| (i, o.clone()))
            .collect();
        if removed.is_empty() {
            return;
        }
        let affected: std::collections::HashSet<_> = ids
            .iter()
            .flat_map(|id| self.links.get(id).into_iter().flatten().copied())
            .filter(|id| !ids.contains(id))
            .collect();
        if affected.is_empty() {
            self.execute(Command::Erase(removed));
            return;
        }
        let primary = removed.iter().map(|(_, o)| o.id()).collect();
        let mut before = removed.clone();
        let mut after = Vec::new();
        for id in affected {
            let Some(ObjectSnapshot::Connector(source)) = self.snapshot(id) else {
                continue;
            };
            let mut c = (**source).clone();
            for end in c.ends_mut() {
                if end.target().is_some_and(|id| ids.contains(&id)) {
                    *end = crate::connector::Endpoint::Free {
                        position: end.position(),
                    };
                }
            }
            let index = self.object_slots[&id];
            let shifted = index - removed.partition_point(|(i, _)| *i < index);
            before.push((index, self.items[index].clone()));
            after.push((shifted, c.into()));
        }
        before.sort_by_key(|(i, _)| *i);
        after.sort_by_key(|(i, _)| *i);
        self.execute_selected(Command::Replace { before, after }, Some(primary));
    }
    pub fn replace_partial(
        &mut self,
        mut replacements: std::collections::HashMap<Uuid, Vec<Stroke>>,
    ) -> bool {
        if replacements.is_empty() {
            return false;
        }
        if replacements
            .keys()
            .any(|id| self.is_locked(*id) || self.parent(*id) != self.editing_group)
        {
            return false;
        }
        let mut ids = self.live_ids.clone();
        if replacements.keys().any(|id| !ids.contains(id)) {
            return false;
        }
        for object in &self.items {
            if let Some(pieces) = replacements.get(&object.id()) {
                let ObjectSnapshot::Stroke(source) = object else {
                    return false;
                };
                if pieces.iter().any(|s| {
                    !self.valid_stroke(s)
                        || s.surface != source.surface
                        || s.pen_type != source.pen_type
                        || s.color != source.color
                        || s.placement != source.placement
                        || s.width != source.width
                        || s.pressure_style != source.pressure_style
                        || s.smoothing != Smoothing::PolylineV1
                        || !ids.insert(s.id)
                }) {
                    return false;
                }
            }
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut output = 0;
        for (index, object) in self.items.iter().enumerate() {
            if let Some(pieces) = replacements.remove(&object.id()) {
                before.push((index, object.clone()));
                for piece in pieces {
                    after.push((output, piece.into()));
                    output += 1;
                }
            } else {
                output += 1;
            }
        }
        self.execute(Command::Replace { before, after });
        true
    }
    #[cfg(test)]
    pub fn insert_objects(&mut self, objects: Vec<Stroke>) -> bool {
        self.insert_snapshots(objects.into_iter().map(Into::into).collect())
    }
    pub fn insert_snapshots(&mut self, mut objects: Vec<ObjectSnapshot>) -> bool {
        if objects.is_empty() {
            return false;
        }
        let replacements: std::collections::HashMap<_, _> =
            objects.iter().map(|o| (o.id(), o.clone())).collect();
        let mut ids = self.live_ids.clone();
        for object in &mut objects {
            if let ObjectSnapshot::Connector(c) = object {
                let mut resolved = (**c).clone();
                self.resolve_connector(&mut resolved, &replacements);
                *object = resolved.into();
            }
            let valid = match &*object {
                ObjectSnapshot::Connector(c) => self
                    .valid_connector(c, |id| replacements.get(&id).or_else(|| self.snapshot(id))),
                other => self.valid_object(other),
            };
            if !valid || !ids.insert(object.id()) {
                return false;
            }
        }
        let mut combined: Vec<_> = self.items.iter().chain(&objects).cloned().collect();
        if let Some(scope) = self.editing_group {
            let owned: std::collections::HashSet<_> = objects
                .iter()
                .filter_map(|o| match o {
                    ObjectSnapshot::Group(g) => Some(&g.children),
                    _ => None,
                })
                .flatten()
                .copied()
                .collect();
            let roots: Vec<_> = objects
                .iter()
                .map(ObjectSnapshot::id)
                .filter(|id| !owned.contains(id))
                .collect();
            if let Some(ObjectSnapshot::Group(g)) = combined.iter_mut().find(|o| o.id() == scope) {
                Arc::make_mut(g).children.extend(roots);
            }
        }
        if !crate::group::validate(&combined) {
            return false;
        }
        let start = self.items.len();
        self.execute(Command::Replace {
            before: Vec::new(),
            after: objects
                .into_iter()
                .enumerate()
                .map(|(i, o)| (start + i, o))
                .collect(),
        });
        true
    }
    pub fn transform_objects(
        &mut self,
        ids: &std::collections::HashSet<Uuid>,
        delta: crate::object::Placement,
    ) -> bool {
        if !delta.valid() || delta.is_identity() || ids.is_empty() {
            return false;
        }
        if ids.iter().any(|id| self.is_locked(*id)) {
            return false;
        }
        let primary = ids.iter().copied().collect();
        let ids = &self.expanded(ids);
        if ids.iter().any(|id| self.is_locked(*id)) {
            return false;
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (index, source) in self
            .items
            .iter()
            .enumerate()
            .filter(|(_, s)| ids.contains(&s.id()))
        {
            let target = match source {
                ObjectSnapshot::Connector(c) => c
                    .transformed_in_group(
                        delta,
                        ids,
                        self.parent(c.id).is_some_and(|p| ids.contains(&p)),
                    )
                    .into(),
                _ => source.transformed(delta),
            };
            if !self.valid_object(&target) {
                return false;
            }
            before.push((index, source.clone()));
            after.push((index, target));
        }
        if before.len() != ids.len() {
            return false;
        }
        if !self.update_links(&mut before, &mut after) {
            return false;
        }
        self.execute_selected(Command::Replace { before, after }, Some(primary));
        true
    }
    /// One atomic mixed-selection formatting transaction. During a continuous
    /// control gesture merge only when the preceding result exactly matches.
    pub fn format_shapes(
        &mut self,
        ids: &std::collections::HashSet<Uuid>,
        patch: crate::shape::StylePatch,
        merge: bool,
    ) -> bool {
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut changed = false;
        for (index, object) in self
            .items
            .iter()
            .enumerate()
            .filter(|(_, o)| ids.contains(&o.id()) && !self.is_locked(o.id()))
        {
            let target = match object {
                ObjectSnapshot::Shape(shape) => {
                    let mut target = (**shape).clone();
                    patch.apply(&mut target);
                    if target == **shape {
                        object.clone()
                    } else {
                        target.into()
                    }
                }
                ObjectSnapshot::Connector(c) => {
                    let mut target = (**c).clone();
                    target.apply_style(patch);
                    if target == **c {
                        object.clone()
                    } else {
                        target.into()
                    }
                }
                _ => continue,
            };
            changed |= target != *object;
            if !self.valid_object(&target) {
                return false;
            }
            before.push((index, object.clone()));
            after.push((index, target));
        }
        if !changed {
            return false;
        }
        let original = if merge && self.cursor == self.history.len() {
            match self.history.last().map(|entry| &entry.command) {
                Some(Command::Replace {
                    before: original,
                    after: previous,
                }) if *previous == before => Some(original.clone()),
                _ => None,
            }
        } else {
            None
        };
        if let Some(original) = original {
            self.history.pop();
            self.cursor -= 1;
            self.execute(Command::Replace {
                before: original,
                after,
            });
        } else {
            self.execute(Command::Replace { before, after });
        }
        true
    }
    /// Canonical text changes share the object journal. Merge only the same
    /// immediately preceding results; callers bound typing/slider sessions.
    pub fn replace_texts(&mut self, texts: Vec<crate::text::TextObject>, merge: bool) -> bool {
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut next = self.items.len();
        for text in texts {
            if self.is_locked(text.id) {
                return false;
            }
            if !seen.insert(text.id) {
                return false;
            }
            let object = ObjectSnapshot::from(text);
            if !self.valid_object(&object) {
                return false;
            }
            let index = if let Some(old) = self.snapshot(object.id()) {
                let (ObjectSnapshot::Text(a), ObjectSnapshot::Text(b)) = (old, &object) else {
                    return false;
                };
                if a.document_id != b.document_id || a.surface != b.surface {
                    return false;
                }
                if old == &object {
                    continue;
                }
                let i = self.object_slots[&object.id()];
                before.push((i, old.clone()));
                i
            } else {
                let i = next;
                next += 1;
                i
            };
            after.push((index, object));
        }
        if after.is_empty() {
            return false;
        }
        before.sort_by_key(|(i, _)| *i);
        after.sort_by_key(|(i, _)| *i);
        let original = if merge && self.cursor == self.history.len() {
            match self.history.last().map(|entry| &entry.command) {
                Some(Command::Replace {
                    before: original,
                    after: previous,
                }) if *previous == before => Some(original.clone()),
                _ => None,
            }
        } else {
            None
        };
        if let Some(original) = original {
            self.history.pop();
            self.cursor -= 1;
            self.execute(Command::Replace { before, after });
            if let Command::Replace { before, .. } = &mut self.history.last_mut().unwrap().command {
                *before = original;
            }
        } else {
            self.execute(Command::Replace { before, after });
        }
        true
    }
    /// Replace rich-text adapters in one journal entry. A standalone adapter
    /// updates its TextObject; an adapter whose stable ID names a Shape updates
    /// only that shape's embedded text, retaining geometry/style/relationships.
    pub fn replace_text_adapters(
        &mut self,
        texts: Vec<crate::text::TextObject>,
        vertical: Option<crate::shape::VerticalAlignment>,
        merge: bool,
    ) -> bool {
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut next = self.items.len();
        for text in texts {
            let id = text.id;
            if self.is_locked(id) {
                return false;
            }
            if !seen.insert(id) {
                return false;
            }
            let old = self.snapshot(id);
            let target: ObjectSnapshot = match old {
                Some(ObjectSnapshot::Text(_)) | None => text.into(),
                Some(ObjectSnapshot::Shape(shape)) => {
                    let mut target = (**shape).clone();
                    let mut shape_text = target.text.clone().unwrap_or_default();
                    shape_text.content = text.content.clone();
                    if let Some(value) = vertical {
                        shape_text.vertical_alignment = value;
                    }
                    target.text = Some(shape_text);
                    if target.sticky_note.is_some_and(|note| note.auto_height)
                        && let crate::shape::ShapeGeometry::Box {
                            size,
                            corner_radius,
                            ..
                        } = &target.geometry
                    {
                        let original_geometry = target.geometry.clone();
                        let size = *size;
                        let wanted = (text.height + 28.0).max(size[1]);
                        target.geometry = crate::shape::ShapeGeometry::Box {
                            kind: crate::shape::ShapeKind::RoundedRectangle,
                            size: [size[0], wanted],
                            corner_radius: (*corner_radius).min(wanted * 0.2),
                        };
                        // Keep edits even when Page bounds leave no room to grow;
                        // the renderer marks the retained overflow for the user.
                        if !self.valid_object(&target.clone().into()) {
                            target.geometry = original_geometry;
                        }
                    }
                    target.into()
                }
                Some(_) => return false,
            };
            if !self.valid_object(&target) {
                return false;
            }
            if old == Some(&target) {
                continue;
            }
            let index = if let Some(old) = old {
                let index = self.object_slots[&id];
                before.push((index, old.clone()));
                index
            } else {
                let index = next;
                next += 1;
                index
            };
            after.push((index, target));
        }
        if after.is_empty() {
            return false;
        }
        if !self.update_links(&mut before, &mut after) {
            return false;
        }
        before.sort_by_key(|(i, _)| *i);
        after.sort_by_key(|(i, _)| *i);
        let original = if merge && self.cursor == self.history.len() {
            match self.history.last().map(|entry| &entry.command) {
                Some(Command::Replace {
                    before: original,
                    after: previous,
                }) if *previous == before => Some(original.clone()),
                _ => None,
            }
        } else {
            None
        };
        if let Some(original) = original {
            self.history.pop();
            self.cursor -= 1;
            self.execute(Command::Replace { before, after });
            if let Command::Replace { before, .. } = &mut self.history.last_mut().unwrap().command {
                *before = original;
            }
        } else {
            self.execute(Command::Replace { before, after });
        }
        true
    }
    pub fn fit_text(&self, text: &mut crate::text::TextObject) {
        if let Some(page) = self.bounds(text.surface) {
            let origin = text.placement.apply([0.; 2]);
            let edge = text.placement.apply([text.width, 0.]);
            let up = text.placement.apply([0., 1.]);
            let direction: [f64; 2] = std::array::from_fn(|i| up[i] - origin[i]);
            let mut limit = text.height;
            for base in [origin, edge] {
                for axis in 0..2 {
                    if direction[axis] > 1e-10 {
                        limit = limit.min((page[axis] - base[axis]) / direction[axis]);
                    } else if direction[axis] < -1e-10 {
                        limit = limit.min(-base[axis] / direction[axis]);
                    }
                }
            }
            text.height = limit.max(1.);
        }
    }
    pub fn replace_shape(&mut self, shape: crate::shape::Shape) -> bool {
        if self.is_locked(shape.id) {
            return false;
        }
        let Some(index) = self.items.iter().position(|o| o.id() == shape.id) else {
            return false;
        };
        let ObjectSnapshot::Shape(old) = &self.items[index] else {
            return false;
        };
        if old.surface != shape.surface || old.document_id != shape.document_id || **old == shape {
            return false;
        }
        let target = ObjectSnapshot::from(shape);
        if !self.valid_object(&target) {
            return false;
        }
        let mut before = vec![(index, self.items[index].clone())];
        let mut after = vec![(index, target)];
        let primary = before.iter().map(|(_, o)| o.id()).collect();
        if !self.update_links(&mut before, &mut after) {
            return false;
        }
        self.execute_selected(Command::Replace { before, after }, Some(primary));
        true
    }
    fn execute(&mut self, command: Command) {
        self.execute_selected(command, None);
    }
    fn execute_selected(&mut self, mut command: Command, selection: Option<Vec<Uuid>>) {
        let organized = self
            .items
            .iter()
            .any(|o| matches!(o, ObjectSnapshot::Group(_)))
            || matches!(&command, Command::Replace { after, .. } if after.iter().any(|(_, o)| matches!(o, ObjectSnapshot::Group(_))));
        let original = organized.then(|| self.items.clone());
        self.clear_routing_hints();
        self.history.truncate(self.cursor);
        let append = matches!(&command,Command::Replace {before,after} if before.is_empty()&&after.iter().enumerate().all(|(i,(index,_))|*index==self.items.len()+i));
        command.apply(&mut self.items);
        if let Some(original) = original {
            if let Some(scope) = self.editing_group {
                let old: std::collections::HashSet<_> =
                    original.iter().map(ObjectSnapshot::id).collect();
                let owned: std::collections::HashSet<_> = self
                    .items
                    .iter()
                    .filter_map(|o| match o {
                        ObjectSnapshot::Group(g) => Some(&g.children),
                        _ => None,
                    })
                    .flatten()
                    .copied()
                    .collect();
                let added: Vec<_> = self
                    .items
                    .iter()
                    .map(ObjectSnapshot::id)
                    .filter(|id| !old.contains(id) && !owned.contains(id))
                    .collect();
                if !added.is_empty() {
                    let children = self
                        .items
                        .iter()
                        .find_map(|o| match o {
                            ObjectSnapshot::Group(g) if g.id == scope => Some(g.children.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let scope_ids: std::collections::HashSet<_> =
                        children.into_iter().chain(added).collect();
                    let order = self
                        .items
                        .iter()
                        .map(ObjectSnapshot::id)
                        .filter(|id| scope_ids.contains(id))
                        .collect();
                    if let Some(ObjectSnapshot::Group(g)) =
                        self.items.iter_mut().find(|o| o.id() == scope)
                    {
                        Arc::make_mut(g).children = order;
                    }
                }
            }
            self.normalize_groups();
            let before = original
                .iter()
                .enumerate()
                .filter(|(i, o)| self.items.get(*i) != Some(*o))
                .map(|(i, o)| (i, o.clone()))
                .collect();
            let after = self
                .items
                .iter()
                .enumerate()
                .filter(|(i, o)| original.get(*i) != Some(*o))
                .map(|(i, o)| (i, o.clone()))
                .collect();
            command = Command::Replace { before, after };
            self.refresh_ink_view();
        } else if append {
            let Command::Replace { after, .. } = &command else {
                unreachable!()
            };
            for &(index, _) in after {
                self.index_object(index);
            }
            self.live_ids.extend(after.iter().map(|(_, o)| o.id()));
            self.strokes.extend(after.iter().filter_map(|(_, o)| {
                if let ObjectSnapshot::Stroke(s) = o {
                    Some(s.clone())
                } else {
                    None
                }
            }));
        } else {
            self.refresh_ink_view();
        }
        self.history.push(HistoryEntry {
            command,
            undo_selection: selection.clone(),
            selection,
        });
        self.cursor += 1;
        self.revision += 1;
    }
    pub fn undo(&mut self) -> Vec<Uuid> {
        self.clear_routing_hints();
        if !self.can_undo() {
            return Vec::new();
        }
        self.cursor -= 1;
        self.revision += 1;
        let entry = &self.history[self.cursor];
        let command = &entry.command;
        command.revert(&mut self.items);
        let ids = entry
            .undo_selection
            .clone()
            .unwrap_or_else(|| match command {
                Command::Erase(objects)
                | Command::Replace {
                    before: objects, ..
                } => objects.iter().map(|(_, s)| s.id()).collect(),
            });
        self.refresh_ink_view();
        ids
    }
    pub fn redo(&mut self) -> Vec<Uuid> {
        self.clear_routing_hints();
        if !self.can_redo() {
            return Vec::new();
        }
        let entry = &self.history[self.cursor];
        let command = &entry.command;
        command.apply(&mut self.items);
        let ids = entry.selection.clone().unwrap_or_else(|| match command {
            Command::Erase(_) => Vec::new(),
            Command::Replace { after, .. } => after.iter().map(|(_, s)| s.id()).collect(),
        });
        self.cursor += 1;
        self.revision += 1;
        self.refresh_ink_view();
        ids
    }
}

/// Midpoint quadratic interpolation stays inside the samples' convex hull (A4
/// bounds included). Original points remain editable and available for M2.
pub fn smooth(points: &[InkPoint]) -> Vec<[f64; 2]> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut result = vec![first.position];
    let mut start = first.position;
    for pair in points.windows(2) {
        let control = pair[0].position;
        let end = std::array::from_fn(|i| (pair[0].position[i] + pair[1].position[i]) * 0.5);
        let distance = distance(start, control) + distance(control, end);
        let steps = (distance / 2.0).ceil().clamp(1.0, 32.0) as usize;
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            result.push(std::array::from_fn(|i| {
                (1.0 - t).powi(2) * start[i] + 2.0 * (1.0 - t) * t * control[i] + t * t * end[i]
            }));
        }
        start = end;
    }
    if let Some(last) = points.last() {
        result.push(last.position);
    }
    result
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn point_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let len2 = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / len2).clamp(0.0, 1.0)
    };
    distance(p, std::array::from_fn(|i| a[i] + t * (b[i] - a[i])))
}
/// Swept eraser hit-test against the same smoothed centerline as rendering.
pub fn hit_path(path: &[[f64; 2]], from: [f64; 2], to: [f64; 2], radius: f64) -> bool {
    if path.len() == 1 {
        return point_segment(path[0], from, to) <= radius;
    }
    path.windows(2).any(|pair| {
        let (a, b) = (pair[0], pair[1]);
        if (0..2).any(|i| {
            (a[i].max(b[i]) + radius).next_up() < from[i].min(to[i])
                || (from[i].max(to[i]) + radius).next_up() < a[i].min(b[i])
        }) {
            return false;
        }
        let intersects = cross(a, b, from) * cross(a, b, to) < 0.0
            && cross(from, to, a) * cross(from, to, b) < 0.0;
        intersects
            || [
                point_segment(from, a, b),
                point_segment(to, a, b),
                point_segment(a, from, to),
                point_segment(b, from, to),
            ]
            .into_iter()
            .any(|d| d <= radius)
    })
}

// Original M1 exact test retained as an independent regression/benchmark oracle.
#[cfg(test)]
pub(crate) fn reference_hit_path(
    path: &[[f64; 2]],
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
) -> bool {
    if path.len() == 1 {
        return point_segment(path[0], from, to) <= radius;
    }
    path.windows(2).any(|pair| {
        let (a, b) = (pair[0], pair[1]);
        let intersects = cross(a, b, from) * cross(a, b, to) < 0.0
            && cross(from, to, a) * cross(from, to, b) < 0.0;
        intersects
            || [
                point_segment(from, a, b),
                point_segment(to, a, b),
                point_segment(a, from, to),
                point_segment(b, from, to),
            ]
            .into_iter()
            .any(|d| d <= radius)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentKind;
    use std::collections::HashSet;

    fn stroke(data: &DocumentData, surface: Surface, points: &[[f64; 2]]) -> Stroke {
        let mut stroke = Stroke::new(data.id(), surface, [20, 80, 180, 255], 2.5);
        for point in points {
            stroke.push(*point);
        }
        stroke
    }
    #[test]
    fn pressure_normalization_mapping_and_legacy_compatibility() {
        assert_eq!(normalize_pressure(None), None);
        assert_eq!(normalize_pressure(Some(f32::NAN)), None);
        assert_eq!(normalize_pressure(Some(f32::INFINITY)), None);
        assert_eq!(normalize_pressure(Some(-1.0)), Some(0.0));
        assert_eq!(normalize_pressure(Some(2.0)), Some(1.0));
        assert_eq!(mix_pressure(Some(0.0), Some(1.0), 0.25), Some(0.25));
        assert_eq!(mix_pressure(None, Some(1.0), 0.5), None);
        let point: InkPoint = serde_json::from_str(r#"{"position":[1,2],"pressure":1.0}"#).unwrap();
        assert_eq!(point.pressure, Some(1.0));
        let missing: InkPoint = serde_json::from_str(r#"{"position":[1,2]}"#).unwrap();
        assert_eq!(missing.pressure, None);
        for strength in [0, 25, 50, 100] {
            let style = PressureStyle {
                enabled: true,
                strength,
                minimum: 20,
            };
            assert_eq!(style.factor(None), 1.0);
            assert_eq!(style.factor(Some(1.0)), 1.0);
            assert!(style.factor(Some(0.0)) <= style.factor(Some(0.5)));
            assert!((0.19..=1.0).contains(&style.factor(Some(0.0))));
        }
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let ink = stroke(&data, Surface::Canvas, &[[1.0, 2.0]]);
        let mut json = serde_json::to_value(&ink).unwrap();
        json.as_object_mut().unwrap().remove("pressure_style");
        let old: Stroke = serde_json::from_value(json).unwrap();
        assert!(!old.pressure_style.enabled);
    }

    #[test]
    fn strokes_have_identity_pressure_and_serializable_vector_samples() {
        let data = DocumentData::blank(DocumentKind::Page);
        let a = stroke(&data, Surface::Page(0), &[[1.0, 2.0], [30.0, 40.0]]);
        let b = stroke(&data, Surface::Page(0), &[[1.0, 2.0]]);
        assert_ne!(a.id, b.id);
        assert_eq!(a.points[0].pressure, None);
        let bytes = serde_json::to_vec(&a).unwrap();
        assert_eq!(serde_json::from_slice::<Stroke>(&bytes).unwrap(), a);
        let mut drawing = Drawing::new(&data);
        assert!(drawing.add(a.clone()));
        assert!(!drawing.add(a));
        assert!(drawing.dirty());
    }
    #[test]
    fn adaptive_metadata_retains_raw_pressure_style_and_legacy_geometry() {
        let data = DocumentData::blank(DocumentKind::Page);
        let mut ink = stroke(
            &data,
            Surface::Page(0),
            &[[2.0, 4.0], [10.0, 4.2], [18.0, 4.0]],
        );
        ink.points[1].pressure = Some(0.4);
        ink.smoothing = Smoothing::AdaptiveV1 { strength: 75 };
        let original = ink.clone();
        let path = ink.path();
        assert_eq!(ink, original);
        let restored: Stroke = serde_json::from_slice(&serde_json::to_vec(&ink).unwrap()).unwrap();
        assert_eq!(restored, ink);
        assert_eq!(restored.path(), path);
        let mut value = serde_json::to_value(&ink).unwrap();
        value.as_object_mut().unwrap().remove("smoothing");
        let legacy: Stroke = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.smoothing, Smoothing::Legacy);
        assert_eq!(legacy.path(), smooth(&ink.points));
        ink.smoothing = Smoothing::AdaptiveV1 { strength: 101 };
        assert!(!Drawing::new(&data).add(ink));
    }

    #[test]
    fn transformations_round_trip_at_zoom_pan_resize_and_large_origins() {
        for scale in [0.1, 1.0, 2.75, 8.0] {
            for origin in [[30.0, 50.0], [900.0, 400.0]] {
                let transform = Transform {
                    origin,
                    center: [1e12, -1e12],
                    scale,
                };
                let world = [1e12 + 125.0, -1e12 - 20.0];
                let screen = transform.to_screen(world);
                let restored = transform.to_document(screen);
                assert!((world[0] - restored[0]).abs() < 0.001);
                assert!((world[1] - restored[1]).abs() < 0.001);
                // DPI does not enter a logical-coordinate transform.
                for dpi in [1.0, 1.5, 2.0] {
                    let physical = screen.map(|v| v * dpi);
                    assert_eq!(transform.to_document(physical.map(|v| v / dpi)), restored);
                }
            }
        }
    }
    #[test]
    fn page_boundaries_and_document_ownership_are_enforced() {
        let page = DocumentData::blank(DocumentKind::Page);
        let board = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&page);
        for (surface, point) in [
            (Surface::Canvas, [0.0, 0.0]),
            (Surface::Page(1), [0.0, 0.0]),
            (Surface::Page(0), [-0.1, 10.0]),
            (Surface::Page(0), [600.0, 10.0]),
            (Surface::Page(0), [10.0, f64::NAN]),
        ] {
            assert!(!drawing.add(stroke(&page, surface, &[point])));
        }
        assert!(!drawing.add(stroke(&board, Surface::Page(0), &[[1.0, 1.0]])));
        assert!(drawing.add(stroke(
            &page,
            Surface::Page(0),
            &[[0.0, 0.0], [210.0 * POINTS_PER_MM, 297.0 * POINTS_PER_MM]]
        )));
        let path = smooth(&drawing.strokes()[0].points);
        assert!(path.iter().all(|p| drawing.contains(Surface::Page(0), *p)));
        let mut whiteboard = Drawing::new(&board);
        assert!(whiteboard.add(stroke(&board, Surface::Canvas, &[[-1e12, 1e12]])));
        assert_eq!(drawing.strokes().len(), 1);
    }
    #[test]
    fn swept_eraser_hits_segments_dots_and_stroke_width() {
        let path = vec![[0.0, 0.0], [100.0, 0.0]];
        assert!(hit_path(&path, [50.0, -100.0], [50.0, 100.0], 1.0));
        assert!(hit_path(&path, [40.0, 4.0], [60.0, 4.0], 5.0));
        assert!(!hit_path(&path, [40.0, 7.0], [60.0, 7.0], 5.0));
        assert!(hit_path(&[[3.0, 4.0]], [3.0, 4.0], [3.0, 4.0], 1.0));
        assert!(!hit_path(&[], [0.0, 0.0], [1.0, 1.0], 10.0));
    }
    #[test]
    fn commands_undo_redo_multiple_erasures_in_order_and_branch() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&data);
        let strokes: Vec<_> = (0..4)
            .map(|i| stroke(&data, Surface::Canvas, &[[i as f64, 0.0]]))
            .collect();
        for s in &strokes {
            assert!(drawing.add(s.clone()));
        }
        drawing.erase(&HashSet::from([strokes[0].id, strokes[2].id]));
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![strokes[1].id, strokes[3].id]
        );
        drawing.undo();
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            strokes.iter().map(|s| s.id).collect::<Vec<_>>()
        );
        drawing.redo();
        drawing.undo();
        for _ in 0..4 {
            drawing.undo();
        }
        assert!(!drawing.dirty());
        assert!(!drawing.can_undo());
        for _ in 0..4 {
            drawing.redo();
        }
        assert_eq!(drawing.strokes().len(), 4);
        drawing.undo();
        drawing.add(stroke(&data, Surface::Canvas, &[[9.0, 9.0]]));
        assert!(!drawing.can_redo());
    }
    #[test]
    fn partial_replace_keeps_interleaved_order_and_rejects_invalid_pieces_atomically() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&data);
        for i in 0..6 {
            drawing.add(stroke(
                &data,
                Surface::Canvas,
                &[[i as f64, 0.0], [i as f64, 100.0]],
            ));
        }
        let original = drawing.strokes().to_vec();
        let piece = |source: &Stroke, start, end| {
            let mut p = Stroke::new(data.id(), source.surface, source.color, source.width);
            p.smoothing = Smoothing::PolylineV1;
            p.push([source.points[0].position[0], start]);
            p.push([source.points[0].position[0], end]);
            p
        };
        let a = piece(&original[1], 0.0, 30.0);
        let b = piece(&original[1], 70.0, 100.0);
        let c = piece(&original[3], 0.0, 80.0);
        let expected = vec![
            original[0].id,
            a.id,
            b.id,
            original[2].id,
            c.id,
            original[5].id,
        ];
        let mut invalid = a.clone();
        invalid.color = [0, 0, 0, 0];
        let revision = drawing.revision();
        assert!(!drawing.replace_partial(std::collections::HashMap::from([(
            original[1].id,
            vec![invalid]
        )])));
        assert_eq!(drawing.revision(), revision);
        assert_eq!(drawing.strokes(), original);
        let mut reused = a.clone();
        reused.id = original[1].id;
        assert!(!drawing.replace_partial(std::collections::HashMap::from([(
            original[1].id,
            vec![reused]
        )])));
        assert!(drawing.replace_partial(std::collections::HashMap::from([
            (original[1].id, vec![a, b]),
            (original[3].id, vec![c]),
            (original[4].id, vec![])
        ])));
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            expected
        );
        drawing.undo();
        assert_eq!(drawing.strokes(), original);
        drawing.redo();
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            expected
        );
        drawing.undo();
        drawing.add(stroke(&data, Surface::Canvas, &[[12.0, 12.0]]));
        assert!(!drawing.can_redo());
    }

    #[test]
    fn histories_are_isolated_between_documents() {
        let a = DocumentData::blank(DocumentKind::Whiteboard);
        let b = DocumentData::blank(DocumentKind::Whiteboard);
        let mut one = Drawing::new(&a);
        let mut two = Drawing::new(&b);
        one.add(stroke(&a, Surface::Canvas, &[[10.0, 20.0]]));
        two.add(stroke(&b, Surface::Canvas, &[[30.0, 40.0]]));
        one.undo();
        assert!(one.strokes().is_empty());
        assert_eq!(two.strokes().len(), 1);
        assert!(!two.add(stroke(&a, Surface::Canvas, &[[0.0, 0.0]])));
        assert!(two.dirty());
    }
}

#[path = "group_commands.rs"]
mod organization;

#[path = "alignment_commands.rs"]
mod alignment_commands;
