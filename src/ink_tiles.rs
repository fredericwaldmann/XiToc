//! Retain per-stroke coverage in viewport tiles. Padded geometry supplies AA
//! on both sides of a tile boundary before triangles are clipped to the tile.
use super::{Contour, Stroke, StrokePiece, Transform, coverage_mesh, flat_range, round_contours};
use egui::{Color32, Painter, Rect, pos2};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::Arc,
};

const TILE_POINTS: f32 = 128.0;
const STABLE_BATCH: usize = 24;
type Cell = (i32, i32);
type Coverage = Vec<Vec<Contour>>;

#[derive(Default)]
struct Tile {
    stable: Coverage,
    mesh: Arc<egui::Mesh>,
    alpha: Vec<u8>,
}

pub(in crate::canvas) struct Tiles {
    clip: Rect,
    dpi: f32,
    size: f32,
    stable_end: usize,
    tail: BTreeSet<Cell>,
    tiles: BTreeMap<Cell, Tile>,
    tint: Color32,
    dirty_from: Option<usize>,
    live: bool,
}

impl Tiles {
    pub(in crate::canvas) fn new(clip: Rect, dpi: f32, tint: Color32, live: bool) -> Self {
        Self {
            clip,
            dpi,
            size: (TILE_POINTS * dpi).round().max(1.0) / dpi,
            stable_end: 0,
            tail: BTreeSet::new(),
            tiles: BTreeMap::new(),
            tint,
            dirty_from: Some(usize::MAX),
            live,
        }
    }

    pub(in crate::canvas) fn clip(&self) -> Rect {
        self.clip
    }

    pub(in crate::canvas) fn invalidate_tail(&mut self, changed: usize) {
        let segment = changed.saturating_sub(1);
        self.dirty_from = Some(self.dirty_from.map_or(segment, |old| old.min(segment)));
    }

    pub(in crate::canvas) fn finish(&mut self) {
        self.live = false;
        if self.dirty_from.is_none() {
            self.release_working_coverage();
        }
    }

    fn release_working_coverage(&mut self) {
        self.tail.clear();
        for tile in self.tiles.values_mut() {
            tile.stable = Vec::new();
        }
    }

    pub(in crate::canvas) fn prepare(
        &mut self,
        path: StrokePiece<'_>,
        transform: Transform,
        stroke: &Stroke,
        tint: Color32,
    ) {
        let dirty = self.dirty_from.take();
        let Some(dirty) = dirty else {
            if self.tint != tint {
                for tile in self.tiles.values_mut() {
                    super::tint(Arc::make_mut(&mut tile.mesh), tint, &tile.alpha);
                }
                self.tint = tint;
            }
            return;
        };
        let end = path.0.len() - 1;
        let stable = dirty.min(end) / STABLE_BATCH * STABLE_BATCH;
        if stable < self.stable_end {
            self.stable_end = 0;
            self.tiles.clear();
            self.tail.clear();
        }
        let mut changed = BTreeSet::new();
        if stable > self.stable_end {
            let bins = self.bin(path, transform, stroke, self.stable_end..stable);
            for (cell, contours) in bins {
                let tile = self.tiles.entry(cell).or_default();
                tile.stable = merge(&tile.stable, &contours);
                changed.insert(cell);
            }
            self.stable_end = stable;
        }
        changed.append(&mut self.tail);
        if self.tint != tint {
            for tile in self.tiles.values_mut() {
                super::tint(Arc::make_mut(&mut tile.mesh), tint, &tile.alpha);
            }
            self.tint = tint;
        }
        let mut tail = if stroke.pen_type == crate::pen::PenType::Highlighter
            && end > 0
            && path.0.iter().all(|point| *point == path.0[0])
        {
            let pressure = path
                .1
                .iter()
                .copied()
                .max_by(|a, b| {
                    stroke
                        .pressure_style
                        .factor(*a)
                        .total_cmp(&stroke.pressure_style.factor(*b))
                })
                .unwrap();
            self.bin((&path.0[..1], &[pressure]), transform, stroke, 0..0)
        } else {
            self.bin(path, transform, stroke, self.stable_end..end)
        };
        self.tail.extend(tail.keys().copied());
        changed.extend(self.tail.iter().copied());
        for cell in changed {
            let rect = self.rect(cell);
            let tile = self.tiles.entry(cell).or_default();
            let contours = tail.remove(&cell).unwrap_or_default();
            let mut mesh = if contours.is_empty() {
                coverage_mesh(&tile.stable, self.dpi)
            } else {
                coverage_mesh(&merge(&tile.stable, &contours), self.dpi)
            };
            clip_mesh(&mut mesh, rect);
            tile.alpha.clear();
            tile.alpha.extend(mesh.vertices.iter().map(|v| v.color.a()));
            super::tint(&mut mesh, tint, &tile.alpha);
            tile.mesh = Arc::new(mesh);
        }
        self.tiles.retain(|_, tile| !tile.mesh.is_empty());
        if !self.live {
            self.release_working_coverage();
        }
    }

    fn bin(
        &self,
        (points, pressures): StrokePiece<'_>,
        transform: Transform,
        stroke: &Stroke,
        range: Range<usize>,
    ) -> BTreeMap<Cell, Vec<Contour>> {
        let mut contours = Vec::new();
        let flat = stroke.pen_type == crate::pen::PenType::Highlighter;
        if flat && points.len() > 1 {
            for start in (range.start..range.end).step_by(STABLE_BATCH) {
                flat_range(
                    points,
                    pressures,
                    transform,
                    stroke,
                    0.15 / self.dpi,
                    start..(start + STABLE_BATCH).min(range.end),
                    &mut contours,
                );
            }
        } else if range.start < points.len() - 1 || points.len() == 1 {
            for start in (range.start..range.end.max(range.start + 1)).step_by(STABLE_BATCH) {
                let end = (start + STABLE_BATCH).min(range.end);
                let points = &points[start..=end];
                let pressures = &pressures[start..=end];
                let samples: Vec<_> = points
                    .iter()
                    .zip(pressures)
                    .map(|(&point, &force)| {
                        (
                            super::screen_point(transform, point),
                            super::nib_radius(stroke, transform, force),
                        )
                    })
                    .collect();
                let kept = super::super::simplify_swept_disks(&samples, 0.2 / self.dpi);
                let path: Vec<_> = kept.iter().map(|&i| points[i]).collect();
                let force: Vec<_> = kept.iter().map(|&i| pressures[i]).collect();
                round_contours(&path, &force, transform, stroke, self.dpi, &mut contours);
            }
        }
        let mut bins = BTreeMap::<Cell, Vec<Contour>>::new();
        let halo = 2.0 / self.dpi;
        let index = |p: f32| (p / self.size).floor() as i32;
        for contour in contours {
            let mut bounds = Rect::NOTHING;
            for &[x, y] in &contour {
                bounds.extend_with(pos2(x as f32, y as f32));
            }
            let visible = bounds.expand(halo).intersect(self.clip);
            if visible.is_negative() {
                continue;
            }
            for y in index(visible.min.y)..=index(visible.max.y) {
                for x in index(visible.min.x)..=index(visible.max.x) {
                    let cell = (x, y);
                    let rect = self.rect(cell).expand(halo);
                    let clipped = if rect.contains_rect(bounds) {
                        contour.clone()
                    } else {
                        clip_polygon(&contour, rect)
                    };
                    if clipped.len() >= 3 {
                        bins.entry(cell).or_default().push(clipped);
                    }
                }
            }
        }
        bins
    }

    fn rect(&self, (x, y): Cell) -> Rect {
        Rect::from_min_max(
            pos2(x as f32 * self.size, y as f32 * self.size),
            pos2((x + 1) as f32 * self.size, (y + 1) as f32 * self.size),
        )
    }

    #[cfg(test)]
    pub(in crate::canvas) fn meshes(&self) -> impl Iterator<Item = (Rect, &Arc<egui::Mesh>)> {
        self.tiles
            .iter()
            .map(|(&cell, tile)| (self.rect(cell), &tile.mesh))
    }

    pub(in crate::canvas) fn paint(&self, painter: &Painter) {
        for tile in self.tiles.values() {
            painter.add(egui::Shape::Mesh(tile.mesh.clone()));
        }
    }
}

/// Clip triangles after triangulation so their original antialias
/// interpolation survives and egui can batch all tiles under one scissor.
fn clip_mesh(mesh: &mut egui::Mesh, rect: Rect) {
    if mesh.is_empty() || rect.contains_rect(mesh.calc_bounds()) {
        return;
    }
    use egui::epaint::Vertex;
    let edges = [
        (0, rect.min.x, 1.0),
        (0, rect.max.x, -1.0),
        (1, rect.min.y, 1.0),
        (1, rect.max.y, -1.0),
    ];
    let indices = std::mem::take(&mut mesh.indices);
    mesh.indices.reserve(indices.len());
    for &triangle in indices.as_chunks::<3>().0 {
        let vertices = triangle.map(|index| mesh.vertices[index as usize]);
        if vertices.iter().all(|vertex| rect.contains(vertex.pos)) {
            mesh.indices.extend(triangle);
            continue;
        }
        let mut input = [vertices[0]; 7];
        let mut output = input;
        input[..3].copy_from_slice(&vertices);
        let mut len = 3;
        for (axis, edge, sign) in edges {
            if len < 3 {
                break;
            }
            let coordinate = |v: Vertex| if axis == 0 { v.pos.x } else { v.pos.y };
            let mut a = input[len - 1];
            let mut aa = coordinate(a);
            let mut written = 0;
            for &b in &input[..len] {
                let bb = coordinate(b);
                let a_in = sign * (aa - edge) >= 0.0;
                let b_in = sign * (bb - edge) >= 0.0;
                if a_in != b_in {
                    let t = (edge - aa) / (bb - aa);
                    let mut pos = a.pos.lerp(b.pos, t);
                    if axis == 0 {
                        pos.x = edge;
                    } else {
                        pos.y = edge;
                    }
                    let alpha = egui::lerp(f32::from(a.color.a())..=f32::from(b.color.a()), t);
                    output[written] = Vertex {
                        pos,
                        uv: a.uv,
                        color: Color32::from_white_alpha(alpha.round() as u8),
                    };
                    written += 1;
                }
                if b_in {
                    output[written] = b;
                    written += 1;
                }
                (a, aa) = (b, bb);
            }
            std::mem::swap(&mut input, &mut output);
            len = written;
        }
        if len >= 3 {
            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend_from_slice(&input[..len]);
            for i in 1..len as u32 - 1 {
                mesh.indices.extend([base, base + i, base + i + 1]);
            }
        }
    }
}

fn merge(stable: &Coverage, contours: &[Contour]) -> Coverage {
    if stable.is_empty() {
        return super::union(contours);
    }
    // Keep the previous union and incoming primitives in separate operands:
    // a union output may have different winding from the incoming polygons.
    stable.overlay_as::<i64>(&contours, OverlayRule::Union, FillRule::NonZero)
}

fn clip_polygon(points: &Contour, rect: Rect) -> Contour {
    let mut input = points.clone();
    let mut output = Vec::new();
    for (axis, edge, sign) in [
        (0, rect.min.x, 1.0),
        (0, rect.max.x, -1.0),
        (1, rect.min.y, 1.0),
        (1, rect.max.y, -1.0),
    ] {
        let Some(&last) = input.last() else {
            break;
        };
        let edge = f64::from(edge);
        let mut a = last;
        let mut a_inside = sign * (a[axis] - edge) >= 0.0;
        for &b in &input {
            let b_inside = sign * (b[axis] - edge) >= 0.0;
            if a_inside != b_inside {
                let other = 1 - axis;
                let t = (edge - a[axis]) / (b[axis] - a[axis]);
                let mut hit = [edge; 2];
                hit[other] = a[other] + t * (b[other] - a[other]);
                output.push(hit);
            }
            if b_inside {
                output.push(b);
            }
            a = b;
            a_inside = b_inside;
        }
        std::mem::swap(&mut input, &mut output);
        output.clear();
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::Surface;
    use egui::{Vec2, vec2};

    fn hits(tiles: &Tiles, x: f32, y: f32) -> usize {
        let p = pos2(x, y);
        tiles
            .tiles
            .iter()
            .filter(|(cell, _)| {
                let rect = tiles.rect(**cell);
                p.x >= rect.min.x && p.y >= rect.min.y && p.x < rect.max.x && p.y < rect.max.y
            })
            .map(|(_, tile)| {
                tile.mesh
                    .indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .filter(|triangle| {
                        let [a, b, c] = triangle.map(|i| tile.mesh.vertices[i as usize].pos);
                        let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
                        let winding = [
                            cross(b - a, p - a),
                            cross(c - b, p - b),
                            cross(a - c, p - c),
                        ];
                        winding.iter().all(|v| *v > 0.0) || winding.iter().all(|v| *v < 0.0)
                    })
                    .count()
            })
            .sum()
    }

    #[test]
    fn tail_reuses_distant_tiles_and_commits_without_rebuilding() {
        let mut stroke = Stroke::new(
            uuid::Uuid::new_v4(),
            Surface::Canvas,
            [90, 80, 20, 102],
            60.0,
        );
        stroke.pen_type = crate::pen::PenType::Highlighter;
        let transform = Transform {
            origin: [0.; 2],
            center: [0.; 2],
            scale: 1.,
        };
        let clip = Rect::from_min_size(pos2(0., 0.), vec2(800., 600.));
        let color = super::super::super::ink_color(&stroke);
        let mut tiles = Tiles::new(clip, 1.25, color, true);
        let mut points: Vec<_> = (0..200).map(|i| [20. + f64::from(i) * 2., 70.]).collect();
        let mut pressure = vec![None; points.len()];
        tiles.prepare((&points, &pressure), transform, &stroke, color);
        let stable = tiles.tiles[&(0, 0)].mesh.clone();
        let prior_tail = tiles.tiles[&(3, 0)].mesh.clone();
        tiles.invalidate_tail(points.len());
        points.push([440., 115.]);
        pressure.push(None);
        tiles.prepare((&points, &pressure), transform, &stroke, color);
        assert!(Arc::ptr_eq(&stable, &tiles.tiles[&(0, 0)].mesh));
        assert!(!Arc::ptr_eq(&prior_tail, &tiles.tiles[&(3, 0)].mesh));
        for (rect, mesh) in tiles.meshes() {
            assert!(
                mesh.indices
                    .iter()
                    .all(|&i| rect.contains(mesh.vertices[i as usize].pos))
            );
        }
        for x in [127.99, 128.01, 255.99, 256.01] {
            assert_eq!(hits(&tiles, x, 71.4), 1);
        }
        let last = tiles.tiles[&(3, 0)].mesh.clone();
        tiles.finish();
        assert!(tiles.tiles.values().all(|t| t.stable.is_empty()));
        tiles.prepare((&points, &pressure), transform, &stroke, color);
        assert!(Arc::ptr_eq(&last, &tiles.tiles[&(3, 0)].mesh));
        let tint = Color32::from_rgba_unmultiplied(10, 10, 30, 140);
        tiles.prepare((&points, &pressure), transform, &stroke, tint);
        assert_eq!(last.indices, tiles.tiles[&(3, 0)].mesh.indices);
        assert_eq!(hits(&tiles, 255.99, 71.4), 1);
    }

    #[test]
    fn crossings_across_stable_batches_share_coverage_but_strokes_do_not() {
        let mut stroke = Stroke::new(
            uuid::Uuid::new_v4(),
            Surface::Canvas,
            [90, 80, 20, 102],
            60.0,
        );
        stroke.pen_type = crate::pen::PenType::Highlighter;
        stroke.pressure_style.enabled = true;
        let color = super::super::super::ink_color(&stroke);
        let clip = Rect::from_min_size(pos2(0., 0.), vec2(800., 600.));
        let mut points = Vec::new();
        for (a, b) in [
            ([20., 20.], [230., 230.]),
            ([230., 230.], [230., 20.]),
            ([230., 20.], [20., 230.]),
        ] {
            points.extend((0..64).map(|i| {
                let t = f64::from(i) / 64.;
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            }));
        }
        let pressure: Vec<_> = (0..points.len())
            .map(|i| Some((i % 31) as f32 / 31.))
            .collect();
        for scale in [0.5, 1.0, 2.0, 4.0] {
            let transform = Transform {
                origin: [0.; 2],
                center: [0.; 2],
                scale,
            };
            let mut tiles = Tiles::new(clip, 1., color, true);
            tiles.prepare(
                (&points[..130], &pressure[..130]),
                transform,
                &stroke,
                color,
            );
            tiles.invalidate_tail(130);
            tiles.prepare((&points, &pressure), transform, &stroke, color);
            let x = (124.27 * scale) as f32;
            let y = (125.83 * scale) as f32;
            assert_eq!(hits(&tiles, x, y), 1);
            let mut second = Tiles::new(clip, 1., color, false);
            second.prepare((&points[..65], &pressure[..65]), transform, &stroke, color);
            assert_eq!(hits(&tiles, x, y) + hits(&second, x, y), 2);
        }
    }
}
