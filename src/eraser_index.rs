//! Conservative document-space bounds hierarchy. Queries allocate no storage;
//! exact tests still use the cached rendered smoothed path.
use crate::drawing::{Stroke, Surface};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct Bounds {
    min: [f64; 2],
    max: [f64; 2],
}
impl Bounds {
    fn union(self, other: Self) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i].min(other.min[i])),
            max: std::array::from_fn(|i| self.max[i].max(other.max[i])),
        }
    }
    fn intersects(self, other: Self) -> bool {
        (0..2).all(|i| self.min[i] <= other.max[i] && other.min[i] <= self.max[i])
    }
    fn intersects_sweep(self, sweep: Sweep) -> bool {
        if !self.intersects(sweep.bounds) {
            return false;
        }
        // Clip the centerline to the expanded box. A swept bounding rectangle
        // alone visits nearly the entire document for long diagonal gestures.
        let mut enter: f64 = 0.0;
        let mut exit: f64 = 1.0;
        for i in 0..2 {
            let min = (self.min[i] - sweep.radius).next_down();
            let max = (self.max[i] + sweep.radius).next_up();
            let delta = sweep.to[i] - sweep.from[i];
            if delta == 0.0 {
                if sweep.from[i] < min || sweep.from[i] > max {
                    return false;
                }
            } else {
                let a = (min - sweep.from[i]) / delta;
                let b = (max - sweep.from[i]) / delta;
                enter = enter.max(a.min(b).next_down());
                exit = exit.min(a.max(b).next_up());
                if enter > exit {
                    return false;
                }
            }
        }
        true
    }
}
#[derive(Clone, Copy)]
struct Sweep {
    bounds: Bounds,
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
}
struct Entry {
    bounds: Bounds,
    index: usize,
    surface: Surface,
}
struct Node {
    bounds: Bounds,
    // Preorder nodes: left child follows this node; right child is explicit.
    right: Option<usize>,
    start: usize,
    end: usize,
}
#[derive(Default)]
pub(crate) struct EraserIndex {
    revision: Option<u64>,
    entries: Vec<Entry>,
    nodes: Vec<Node>,
}
impl EraserIndex {
    pub fn sync(&mut self, revision: u64, strokes: &[Arc<Stroke>]) {
        if self.revision == Some(revision) {
            return;
        }
        self.entries.clear();
        self.nodes.clear();
        self.entries
            .extend(strokes.iter().enumerate().map(|(index, stroke)| {
                let mut bounds = Bounds {
                    min: [f64::INFINITY; 2],
                    max: [f64::NEG_INFINITY; 2],
                };
                for point in &stroke.points {
                    for i in 0..2 {
                        bounds.min[i] = bounds.min[i].min(point.position[i]);
                        bounds.max[i] = bounds.max[i].max(point.position[i]);
                    }
                }
                // Smoothing lies in the raw samples' convex hull. Include ink width.
                for i in 0..2 {
                    bounds.min[i] = (bounds.min[i] - stroke.width * 0.5).next_down();
                    bounds.max[i] = (bounds.max[i] + stroke.width * 0.5).next_up();
                }
                Entry {
                    bounds,
                    index,
                    surface: stroke.surface,
                }
            }));
        if !self.entries.is_empty() {
            Self::build(&mut self.nodes, &mut self.entries, 0);
        }
        self.revision = Some(revision);
    }
    fn build(nodes: &mut Vec<Node>, entries: &mut [Entry], start: usize) -> usize {
        let bounds = entries
            .iter()
            .skip(1)
            .fold(entries[0].bounds, |b, e| b.union(e.bounds));
        let node = nodes.len();
        nodes.push(Node {
            bounds,
            right: None,
            start,
            end: start + entries.len(),
        });
        if entries.len() > 8 {
            let axis = usize::from(bounds.max[1] - bounds.min[1] > bounds.max[0] - bounds.min[0]);
            let mid = entries.len() / 2;
            entries.select_nth_unstable_by(mid, |a, b| {
                (a.bounds.min[axis] * 0.5 + a.bounds.max[axis] * 0.5)
                    .total_cmp(&(b.bounds.min[axis] * 0.5 + b.bounds.max[axis] * 0.5))
            });
            let (left, right) = entries.split_at_mut(mid);
            Self::build(nodes, left, start);
            nodes[node].right = Some(Self::build(nodes, right, start + mid));
        }
        node
    }
    pub fn query(
        &self,
        surface: Surface,
        from: [f64; 2],
        to: [f64; 2],
        radius: f64,
        mut visit: impl FnMut(usize),
    ) {
        if self.nodes.is_empty() {
            return;
        }
        let query = Bounds {
            min: std::array::from_fn(|i| (from[i].min(to[i]) - radius).next_down()),
            max: std::array::from_fn(|i| (from[i].max(to[i]) + radius).next_up()),
        };
        self.visit(
            0,
            Sweep {
                bounds: query,
                from,
                to,
                radius,
            },
            surface,
            &mut visit,
        );
    }
    fn visit(&self, index: usize, query: Sweep, surface: Surface, visit: &mut impl FnMut(usize)) {
        let node = &self.nodes[index];
        if !node.bounds.intersects_sweep(query) {
            return;
        }
        if let Some(right) = node.right {
            self.visit(index + 1, query, surface, visit);
            self.visit(right, query, surface, visit);
        } else {
            for entry in &self.entries[node.start..node.end] {
                if entry.surface == surface && entry.bounds.intersects_sweep(query) {
                    visit(entry.index);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentData, DocumentKind};

    #[test]
    fn local_queries_prune_large_documents_and_keep_surface_and_width() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let strokes: Vec<_> = (0..10_000)
            .map(|i| {
                let mut stroke = Stroke::new(data.id(), Surface::Canvas, [0; 4], 24.0);
                stroke.push([
                    1e12 + (i % 100) as f64 * 100.0,
                    -1e12 + (i / 100) as f64 * 100.0,
                ]);
                Arc::new(stroke)
            })
            .collect();
        let mut index = EraserIndex::default();
        index.sync(1, &strokes);
        let mut hits = Vec::new();
        let point = [1e12 + 13.0, -1e12];
        index.query(Surface::Canvas, point, point, 1.0, |i| hits.push(i));
        // Tangency includes stroke half-width, even far from the origin.
        assert_eq!(hits, [0]);
        let mut candidates = 0;
        index.query(
            Surface::Canvas,
            [1e12, -1e12],
            [1e12 + 9900.0, -1e12 + 9900.0],
            1.0,
            |_| candidates += 1,
        );
        assert_eq!(candidates, 100); // diagonal, not its 10,000-stroke bounding box
        let entries = index.entries.as_ptr();
        index.sync(1, &[]); // unchanged revision must reuse the hierarchy
        assert_eq!(index.entries.as_ptr(), entries);
        hits.clear();
        index.query(Surface::Page(0), point, point, 1.0, |i| hits.push(i));
        assert!(hits.is_empty());
        index.sync(2, &[]);
        index.query(Surface::Canvas, point, point, 1.0, |i| hits.push(i));
        assert!(hits.is_empty());
    }
}
