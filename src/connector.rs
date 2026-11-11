//! Semantic magnetic connectors. Attached IDs/ports are authoritative; stored
//! last positions are bounded fallbacks for selection, detachment and clipboard.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
    shape::{Arrowhead, Shape, ShapeGeometry, ShapeKind, ShapeStyle},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Port {
    Top,
    Right,
    Bottom,
    Left,
}
impl Port {
    pub const ALL: [Self; 4] = [Self::Top, Self::Right, Self::Bottom, Self::Left];
    pub fn normal(self, shape: &Shape) -> Point {
        let v = match self {
            Self::Top => [0., -1.],
            Self::Right => [1., 0.],
            Self::Bottom => [0., 1.],
            Self::Left => [-1., 0.],
        };
        // Direction vectors must not subtract translated far-origin points.
        let v: Point = std::array::from_fn(|i| {
            if shape.placement.reflected[i] {
                -v[i]
            } else {
                v[i]
            }
        });
        let (sin, cos) = shape.placement.rotation.sin_cos();
        [cos * v[0] - sin * v[1], sin * v[0] + cos * v[1]]
    }
    pub fn point(self, shape: &Shape) -> Option<Point> {
        let ShapeGeometry::Box {
            kind, size: [w, h], ..
        } = shape.geometry
        else {
            return None;
        };
        let p = match self {
            Self::Top => [w * 0.5, 0.],
            Self::Right => [
                if kind == ShapeKind::Triangle {
                    w * 0.75
                } else {
                    w
                },
                h * 0.5,
            ],
            Self::Bottom => [w * 0.5, h],
            Self::Left => [
                if kind == ShapeKind::Triangle {
                    w * 0.25
                } else {
                    0.
                },
                h * 0.5,
            ],
        };
        Some(shape.placement.apply(p))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Endpoint {
    Free {
        position: Point,
    },
    Attached {
        object_id: ObjectId,
        anchor: Port,
        last_position: Point,
        #[serde(default)]
        automatic: bool,
        #[serde(default)]
        normal: Point,
        #[serde(default)]
        bounds: [Point; 2],
    },
}
impl Endpoint {
    pub fn position(&self) -> Point {
        match self {
            Self::Free { position } => *position,
            Self::Attached { last_position, .. } => *last_position,
        }
    }
    pub fn target(&self) -> Option<ObjectId> {
        match self {
            Self::Attached { object_id, .. } => Some(*object_id),
            _ => None,
        }
    }
    pub fn attached(shape: &Shape, anchor: Port) -> Option<Self> {
        Some(Self::Attached {
            object_id: shape.id,
            anchor,
            last_position: anchor.point(shape)?,
            automatic: false,
            normal: anchor.normal(shape),
            bounds: [shape.geometry_bounds().min, shape.geometry_bounds().max],
        })
    }
    pub fn set_position(&mut self, p: Point) {
        match self {
            Self::Free { position } => *position = p,
            Self::Attached { last_position, .. } => *last_position = p,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Route {
    Straight,
    Orthogonal { rail: Option<Rail> },
    Curved { controls: Option<[Point; 2]> },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rail {
    /// 0: vertical middle segment, 1: horizontal middle segment.
    pub axis: usize,
    /// Offset from the endpoints' midpoint, in document points.
    pub offset: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connector {
    #[serde(default)]
    pub locked: bool,
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub source: Endpoint,
    pub target: Endpoint,
    pub route: Route,
    pub style: ShapeStyle,
    pub end_head: Option<Arrowhead>,
}
impl Connector {
    pub fn new(
        owner: Uuid,
        surface: Surface,
        source: Endpoint,
        target: Endpoint,
        style: ShapeStyle,
    ) -> Self {
        Self {
            locked: false,
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            source,
            target,
            route: Route::Straight,
            style,
            end_head: Some(Arrowhead::default()),
        }
    }
    pub fn ends(&self) -> [&Endpoint; 2] {
        [&self.source, &self.target]
    }
    pub fn ends_mut(&mut self) -> [&mut Endpoint; 2] {
        [&mut self.source, &mut self.target]
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.style.valid()
            && self.route_valid()
            && !(self.source.target().is_some() && self.source.target() == self.target.target())
            && self.ends().iter().all(|e| {
                e.position()
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1e15)
                    && e.target().is_none_or(|id| !id.is_nil() && id != self.id)
            })
            && self.end_head.is_none_or(|h| {
                h.length.is_finite()
                    && h.width.is_finite()
                    && (0.1..=1e4).contains(&h.length)
                    && (0.1..=1e4).contains(&h.width)
            })
    }
    pub fn as_shape(&self) -> Shape {
        let start = self.source.position();
        let end = self.target.position();
        let zero = (end[0] - start[0]).hypot(end[1] - start[1]) < 0.01;
        let mut style = self.style.clone();
        if zero {
            style.start_head = false;
        }
        let geometry = if self.route == Route::Straight {
            ShapeGeometry::Segment {
                start,
                end,
                head: if zero { None } else { self.end_head },
            }
        } else {
            let points = self.path(0.01);
            let tangents = self.tangents(&points);
            ShapeGeometry::Path {
                points,
                tangents,
                head: if zero { None } else { self.end_head },
            }
        };
        Shape {
            locked: self.locked,
            id: self.id,
            document_id: self.document_id,
            surface: self.surface,
            placement: Placement::default(),
            geometry,
            style,
            text: None,
            sticky_note: None,
        }
    }
    /// Formatting retains canonical heads even when coincident endpoints have
    /// no drawable shaft/head. Rendering suppression must never edit the style.
    pub fn formatting_shape(&self) -> Shape {
        let mut copy = self.clone();
        copy.route = Route::Straight;
        let mut shape = copy.as_shape();
        shape.style = self.style.clone();
        if let ShapeGeometry::Segment { head, .. } = &mut shape.geometry {
            *head = self.end_head;
        }
        shape
    }
    pub fn apply_style(&mut self, patch: crate::shape::StylePatch) {
        let mut shape = self.formatting_shape();
        patch.apply(&mut shape);
        self.style = shape.style;
        if let ShapeGeometry::Segment { head, .. } = shape.geometry {
            self.end_head = head;
        }
    }
    /// A selected connector moves its free ends. A selected target supplies its
    /// own transform once; attached ends with unselected targets detach on move.
    pub fn transformed(&self, delta: Placement, selected: &HashSet<ObjectId>) -> Self {
        let mut out = self.clone();
        if let Route::Curved {
            controls: Some(ref mut controls),
        } = out.route
        {
            for p in controls {
                let origin = delta.apply([0.; 2]);
                let moved = delta.apply(*p);
                *p = std::array::from_fn(|i| moved[i] - origin[i]);
            }
        }
        if let Route::Orthogonal {
            rail: Some(ref mut rail),
        } = out.route
        {
            let mut v = [0.; 2];
            v[rail.axis] = rail.offset;
            let origin = delta.apply([0.; 2]);
            let moved = delta.apply(v);
            let d: Point = std::array::from_fn(|i| moved[i] - origin[i]);
            rail.axis = usize::from(d[1].abs() > d[0].abs());
            rail.offset = d[rail.axis];
        }
        for e in out.ends_mut() {
            let position = delta.apply(e.position());
            if e.target().is_some_and(|id| selected.contains(&id)) {
                e.set_position(position);
            } else {
                *e = Endpoint::Free { position };
            }
        }
        out
    }
    pub fn remap(&mut self, ids: &HashMap<ObjectId, ObjectId>) {
        for e in self.ends_mut() {
            if let Endpoint::Attached { object_id, .. } = e {
                if let Some(new) = ids.get(object_id) {
                    *object_id = *new;
                } else {
                    *e = Endpoint::Free {
                        position: e.position(),
                    };
                }
            }
        }
    }
}
impl CanvasObject for Connector {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Connector
    }
    fn document_id(&self) -> Uuid {
        self.document_id
    }
    fn surface(&self) -> Surface {
        self.surface
    }
    fn placement(&self) -> Placement {
        Placement::default()
    }
    fn local_bounds(&self) -> Option<ObjectBounds> {
        self.as_shape().world_bounds()
    }
    fn world_bounds(&self) -> Option<ObjectBounds> {
        self.local_bounds()
    }
    fn geometry_bounds(&self) -> Option<ObjectBounds> {
        Some(self.as_shape().geometry_bounds())
    }
    fn hit_test(&self, p: Point, tolerance: f64) -> bool {
        self.as_shape().hit_test(p, tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{DocumentData, DocumentKind},
        drawing::Drawing,
        object::ObjectSnapshot,
        object_clipboard::ObjectBundle,
        shape::{BorderStyle, StylePatch},
    };
    fn shape(d: &Drawing, s: Surface, kind: ShapeKind, x: f64) -> Shape {
        Shape::create(d.owner(), s, kind, [x, 50.], [x + 80., 110.], false).unwrap()
    }
    fn fixture(kind: DocumentKind) -> (Drawing, [Uuid; 3], [Uuid; 2]) {
        let data = DocumentData::blank(kind);
        let mut d = Drawing::new(&data);
        let s = if kind == DocumentKind::Page {
            Surface::Page(0)
        } else {
            Surface::Canvas
        };
        let shapes = [
            shape(&d, s, ShapeKind::Rectangle, 30.),
            shape(&d, s, ShapeKind::RoundedRectangle, 210.),
            shape(&d, s, ShapeKind::Ellipse, 390.),
        ];
        let nodes = shapes.each_ref().map(|s| s.id);
        for shape in &shapes {
            assert!(d.add_shape(shape.clone()));
        }
        let ids = std::array::from_fn(|i| {
            let c = Connector::new(
                d.owner(),
                s,
                Endpoint::attached(&shapes[i], Port::Right).unwrap(),
                Endpoint::attached(&shapes[i + 1], Port::Left).unwrap(),
                Default::default(),
            );
            let id = c.id;
            assert!(d.add_connector(c));
            id
        });
        (d, nodes, ids)
    }
    fn c(d: &Drawing, id: Uuid) -> &Connector {
        let Some(ObjectSnapshot::Connector(c)) = d.snapshot(id) else {
            panic!()
        };
        c
    }
    fn assert_resolved(d: &Drawing, id: Uuid) {
        for e in c(d, id).ends() {
            if let Endpoint::Attached {
                object_id,
                anchor,
                last_position,
                ..
            } = e
            {
                let Some(ObjectSnapshot::Shape(shape)) = d.snapshot(*object_id) else {
                    panic!()
                };
                assert_eq!(*last_position, anchor.point(shape).unwrap());
            }
        }
    }
    #[test]
    fn border_ports_follow_all_supported_forms_rotation_reflection_and_large_coordinates() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let d = Drawing::new(&data);
        for kind in ShapeKind::ALL.into_iter().filter(|k| !k.segment()) {
            let mut s = shape(&d, Surface::Canvas, kind, 30.);
            for rotation in [0., 0.7, 2.4] {
                for reflected in [[false, false], [true, false], [false, true]] {
                    s.placement.rotation = rotation;
                    s.placement.reflected = reflected;
                    for port in Port::ALL {
                        let point = port.point(&s).unwrap();
                        assert!(s.hit_test(point, 0.1), "{kind:?} {port:?}");
                    }
                    let mut far = s.clone();
                    far.placement.position = [1e12, -1e12];
                    for port in Port::ALL {
                        let point = port.point(&far).unwrap();
                        assert!(point.iter().all(|p| p.is_finite()));
                    }
                }
            }
        }
    }
    #[test]
    fn movement_resize_rotation_flip_and_group_move_resolve_atomically_with_exact_history() {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let (mut d, nodes, edges) = fixture(kind);
            for moving in [
                HashSet::from([nodes[0]]),
                HashSet::from([nodes[1]]),
                HashSet::from([nodes[0], nodes[1]]),
                HashSet::from([nodes[0], nodes[1], edges[0]]),
            ] {
                let before = d.items().to_vec();
                assert!(d.transform_objects(
                    &moving,
                    Placement {
                        position: [10., 10.],
                        ..Default::default()
                    }
                ));
                for edge in edges {
                    assert_resolved(&d, edge);
                }
                let after = d.items().to_vec();
                d.undo();
                assert_eq!(d.items(), before);
                d.redo();
                assert_eq!(d.items(), after);
                d.undo();
            }
            let Some(ObjectSnapshot::Shape(source)) = d.snapshot(nodes[1]) else {
                panic!()
            };
            let mut resized = source.with_corner(2, [325., 155.], false).unwrap();
            resized.placement.rotation = 0.4;
            resized.placement.reflected = [true, false];
            resized.placement.position = [300., 180.];
            let before = d.items().to_vec();
            assert!(d.replace_shape(resized));
            for edge in edges {
                assert_resolved(&d, edge);
            }
            let after = d.items().to_vec();
            d.undo();
            assert_eq!(d.items(), before);
            d.redo();
            assert_eq!(d.items(), after);
        }
    }
    #[test]
    fn deleting_shared_target_detaches_both_incident_edges_and_restores_links_and_order() {
        let (mut d, nodes, edges) = fixture(DocumentKind::Whiteboard);
        let before = d.items().to_vec();
        let positions = [
            c(&d, edges[0]).target.position(),
            c(&d, edges[1]).source.position(),
        ];
        d.erase(&HashSet::from([nodes[1]]));
        assert!(d.snapshot(nodes[1]).is_none());
        assert_eq!(
            c(&d, edges[0]).target,
            Endpoint::Free {
                position: positions[0]
            }
        );
        assert_eq!(
            c(&d, edges[1]).source,
            Endpoint::Free {
                position: positions[1]
            }
        );
        let after = d.items().to_vec();
        d.undo();
        assert_eq!(d.items(), before);
        d.redo();
        assert_eq!(d.items(), after);
        d.undo();
        d.erase(&HashSet::from([nodes[0], edges[0]]));
        assert!(d.snapshot(edges[0]).is_none());
        d.undo();
        assert_eq!(d.items(), before);
    }
    #[test]
    fn endpoints_reconnect_detach_style_and_serialize_without_losing_relationships() {
        let (mut d, nodes, edges) = fixture(DocumentKind::Whiteboard);
        let before = d.items().to_vec();
        let mut edge = c(&d, edges[0]).clone();
        let Some(ObjectSnapshot::Shape(other)) = d.snapshot(nodes[2]) else {
            panic!()
        };
        edge.target = Endpoint::attached(other, Port::Top).unwrap();
        assert!(d.replace_connector(edge));
        assert_resolved(&d, edges[0]);
        let linked = c(&d, edges[0]).clone();
        let mut detached = linked.clone();
        detached.target = Endpoint::Free {
            position: [400., 250.],
        };
        assert!(d.replace_connector(detached));
        d.undo();
        assert_eq!(c(&d, edges[0]), &linked);
        d.undo();
        assert_eq!(d.items(), before);
        for patch in [
            StylePatch::Border([200, 20, 60]),
            StylePatch::BorderOpacity(120),
            StylePatch::Width(4.),
            StylePatch::Pattern(BorderStyle::Dashed),
            StylePatch::DashLength(12.),
            StylePatch::GapLength(6.),
            StylePatch::Heads(true, true),
        ] {
            let before = d.items().to_vec();
            assert!(d.format_shapes(&HashSet::from([edges[0]]), patch, false));
            assert_eq!(c(&d, edges[0]).source.target(), Some(nodes[0]));
            assert_eq!(c(&d, edges[0]).target.target(), Some(nodes[1]));
            let after = d.items().to_vec();
            d.undo();
            assert_eq!(d.items(), before);
            d.redo();
            assert_eq!(d.items(), after);
        }
        let encoded = serde_json::to_string(c(&d, edges[0])).unwrap();
        assert_eq!(
            serde_json::from_str::<Connector>(&encoded).unwrap(),
            *c(&d, edges[0])
        );
        let paths = c(&d, edges[0]).as_shape().contours(0.01);
        assert_eq!(paths[1][0], c(&d, edges[0]).target.position());
    }
    #[test]
    fn clipboard_remaps_internal_targets_and_detaches_uncopied_targets_cross_document() {
        let (mut d, nodes, edges) = fixture(DocumentKind::Whiteboard);
        let original = d.items().to_vec();
        let bundle = ObjectBundle::capture_objects(original.iter().cloned()).unwrap();
        let bundle = ObjectBundle::decode(&bundle.encode().unwrap()).unwrap();
        let ids = bundle.duplicate(&mut d, Surface::Canvas);
        assert_eq!(ids.len(), 5);
        for edge in d.items().iter().filter_map(|o| {
            if let ObjectSnapshot::Connector(c) = o {
                ids.contains(&c.id).then_some(c)
            } else {
                None
            }
        }) {
            for e in edge.ends() {
                assert!(ids.contains(&e.target().unwrap()));
                assert!(!nodes.contains(&e.target().unwrap()));
            }
            assert_resolved(&d, edge.id);
        }
        d.undo();
        assert_eq!(d.items(), original);
        d.redo();
        let data = DocumentData::blank(DocumentKind::Page);
        let mut dest = Drawing::new(&data);
        let pasted = bundle.paste(&mut dest, Surface::Page(0), [280., 300.]);
        assert_eq!(pasted.len(), 5);
        for id in pasted {
            if matches!(dest.snapshot(id), Some(ObjectSnapshot::Connector(_))) {
                assert_resolved(&dest, id);
            }
        }
        let isolated =
            ObjectBundle::capture_objects(std::iter::once(d.snapshot(edges[0]).unwrap().clone()))
                .unwrap();
        let pasted = isolated.paste(&mut dest, Surface::Page(0), [280., 400.]);
        assert_eq!(pasted.len(), 1);
        assert!(
            c(&dest, pasted[0])
                .ends()
                .iter()
                .all(|e| e.target().is_none())
        );
    }
    #[test]
    fn invalid_self_missing_cross_surface_and_future_routes_are_rejected() {
        let (mut d, nodes, edges) = fixture(DocumentKind::Whiteboard);
        let source = c(&d, edges[0]).clone();
        let mut invalid = source.clone();
        invalid.id = Uuid::new_v4();
        invalid.target = invalid.source.clone();
        assert!(!d.add_connector(invalid));
        let mut invalid = source.clone();
        invalid.id = Uuid::new_v4();
        if let Endpoint::Attached { object_id, .. } = &mut invalid.target {
            *object_id = Uuid::new_v4();
        }
        assert!(!d.add_connector(invalid));
        let mut invalid = source.clone();
        invalid.id = Uuid::new_v4();
        invalid.surface = Surface::Page(0);
        assert!(!d.add_connector(invalid));
        let mut json = serde_json::to_value(&source).unwrap();
        json["route"] = "FutureRoute".into();
        assert!(serde_json::from_value::<Connector>(json).is_err());
        assert_eq!(d.links[&nodes[1]].len(), 2);
    }
    #[test]
    fn moving_one_hub_updates_incident_connectors_and_reuses_unrelated_arcs() {
        let (mut d, nodes, edges) = fixture(DocumentKind::Whiteboard);
        let other = d.snapshot(edges[1]).unwrap().clone();
        let Some(ObjectSnapshot::Shape(hub)) = d.snapshot(nodes[0]) else {
            panic!()
        };
        let hub = hub.clone();
        for i in 0..80 {
            let target = shape(
                &d,
                Surface::Canvas,
                ShapeKind::Rectangle,
                600. + i as f64 * 100.,
            );
            let edge = Connector::new(
                d.owner(),
                Surface::Canvas,
                Endpoint::attached(&hub, Port::Right).unwrap(),
                Endpoint::attached(&target, Port::Left).unwrap(),
                Default::default(),
            );
            d.add_shape(target);
            assert!(d.add_connector(edge));
        }
        assert_eq!(d.links[&nodes[0]].len(), 81);
        assert!(d.transform_objects(
            &HashSet::from([nodes[0]]),
            Placement {
                position: [1000., 500.],
                ..Default::default()
            }
        ));
        for id in &d.links[&nodes[0]] {
            assert_resolved(&d, *id);
        }
        let (ObjectSnapshot::Connector(a), Some(ObjectSnapshot::Connector(b))) =
            (&other, d.snapshot(edges[1]))
        else {
            panic!()
        };
        assert!(std::sync::Arc::ptr_eq(a, b));
    }
}
