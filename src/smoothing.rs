//! Local adaptive quadratic smoothing in document points, independent of views.
//! Raw input and pressure stay untouched. Only the last three vertices can change
//! as input grows; the cursor endpoint is always included, without a time filter.
use crate::drawing::InkPoint;

/// Incremental state contains offsets, not a second copy of the input/path.
#[derive(Default)]
pub struct Preview {
    starts: Vec<usize>,
}
impl Preview {
    /// Returns the first possibly modified path point for mesh invalidation.
    pub fn update(
        &mut self,
        points: &[InkPoint],
        strength: u8,
        path: &mut Vec<[f64; 2]>,
    ) -> Option<usize> {
        if points.len() == self.starts.len() {
            return None;
        }
        let start = self.starts.len().saturating_sub(3).min(points.len());
        let changed = self.starts.get(start).copied().unwrap_or(path.len());
        path.truncate(changed);
        self.starts.truncate(start);
        for i in start..points.len() {
            self.starts.push(path.len());
            emit(points, i, strength, path);
        }
        Some(changed)
    }
}

pub fn path(points: &[InkPoint], strength: u8) -> Vec<[f64; 2]> {
    let mut path = Vec::new();
    let _ = Preview::default().update(points, strength, &mut path);
    path
}

fn mix(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
/// Fade smoothing to zero as a turn approaches 75 degrees. This protects
/// deliberate corners, reversals and fine hooks at every strength.
fn allowance(points: &[InkPoint], i: usize) -> f64 {
    if i == 0 || i + 1 == points.len() {
        return 0.0;
    }
    let b = points[i].position;
    let a = points[i - 1].position;
    let c = points[i + 1].position;
    let lengths = distance(a, b) * distance(b, c);
    if lengths == 0.0 {
        return 0.0;
    }
    let cosine = ((b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1])) / lengths;
    const COS_75: f64 = 0.258_819_045_102_520_74;
    ((cosine - COS_75) / (1.0 - COS_75)).clamp(0.0, 1.0)
}
fn filtered(points: &[InkPoint], i: usize, strength: f64) -> [f64; 2] {
    let b = points[i].position;
    let weight = 0.5 * strength * allowance(points, i);
    if weight == 0.0 {
        return b;
    }
    // Relative arithmetic retains precision on distant Whiteboards. Convex
    // weights keep all generated positions within the input bounds (A4 included).
    std::array::from_fn(|axis| {
        b[axis]
            + weight
                * ((points[i - 1].position[axis] - b[axis]) * 0.5
                    + (points[i + 1].position[axis] - b[axis]) * 0.5)
    })
}
fn emit(points: &[InkPoint], i: usize, strength: u8, path: &mut Vec<[f64; 2]>) {
    let amount = f64::from(strength.min(100)) / 100.0;
    let trim = 0.5 * amount * allowance(points, i);
    let b = filtered(points, i, amount);
    if trim == 0.0 {
        path.push(b);
        return;
    }
    let start = mix(b, filtered(points, i - 1, amount), trim);
    let end = mix(b, filtered(points, i + 1, amount), trim);
    path.push(start);
    let steps = ((distance(start, b) + distance(b, end)) / 1.0)
        .ceil()
        .clamp(2.0, 32.0) as usize;
    // de Casteljau form avoids weighted sums of large absolute coordinates.
    for step in 1..=steps {
        let t = step as f64 / steps as f64;
        path.push(mix(mix(start, b, t), mix(b, end, t), t));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn samples(positions: &[[f64; 2]]) -> Vec<InkPoint> {
        positions.iter().copied().map(InkPoint::mouse).collect()
    }
    #[test]
    fn disabled_dots_short_strokes_endpoints_and_sharp_corners() {
        for positions in [
            vec![],
            vec![[2.0, 4.0]],
            vec![[2.0, 4.0], [10.0, 8.0]],
            vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [40.0, 20.0]],
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [1.0, 0.0]],
        ] {
            let points = samples(&positions);
            assert_eq!(path(&points, 0), positions);
            for strength in [25, 50, 75, 100] {
                let smoothed = path(&points, strength);
                assert_eq!(smoothed.first(), positions.first());
                assert_eq!(smoothed.last(), positions.last());
                // All turns in these fixtures are >= 90 degrees, including a
                // tiny hook/reversal: maximum strength must preserve them.
                assert_eq!(smoothed, positions);
            }
        }
    }
    #[test]
    fn increasing_strength_reduces_jitter_and_stays_in_input_bounds() {
        let points: Vec<_> = (0..40)
            .map(|i| InkPoint::mouse([i as f64 * 4.0, if i % 2 == 0 { 0.4 } else { -0.4 }]))
            .collect();
        let legacy = crate::drawing::smooth(&points);
        let legacy_variation: f64 = legacy.windows(2).map(|w| (w[1][1] - w[0][1]).abs()).sum();
        let medium = path(&points, 50);
        let medium_variation: f64 = medium.windows(2).map(|w| (w[1][1] - w[0][1]).abs()).sum();
        assert!(medium_variation < legacy_variation);
        let mut previous = f64::INFINITY;
        for strength in [0, 25, 50, 75, 100] {
            let result = path(&points, strength);
            let variation: f64 = result.windows(2).map(|w| (w[1][1] - w[0][1]).abs()).sum();
            assert!(
                variation < previous,
                "{strength}: {variation} >= {previous}"
            );
            previous = variation;
            assert_eq!(result.first().unwrap(), &points[0].position);
            assert_eq!(result.last().unwrap(), &points.last().unwrap().position);
            assert!(
                result
                    .iter()
                    .all(|p| (0.0..=156.0).contains(&p[0]) && (-0.4..=0.4).contains(&p[1]))
            );
        }
    }
    #[test]
    fn incremental_matches_final_and_only_rewrites_bounded_tail() {
        for strength in [0, 25, 50, 75, 100] {
            let mut input = Vec::new();
            let mut output = Vec::new();
            let mut preview = Preview::default();
            for i in 0..1000 {
                input.push(InkPoint::mouse([i as f64, (i as f64 * 0.1).sin() * 10.0]));
                let old = output.clone();
                let changed = preview.update(&input, strength, &mut output).unwrap();
                assert_eq!(old[..changed], output[..changed]);
                assert!(output.len() - changed <= 4 * 33);
                assert_eq!(output, path(&input, strength));
                assert_eq!(output.last().unwrap(), &input.last().unwrap().position);
                assert!(preview.update(&input, strength, &mut output).is_none());
            }
        }
    }
    #[test]
    fn geometry_is_independent_of_zoom_pan_resize_and_far_origin() {
        use crate::drawing::Transform;
        let points = samples(&[[1.0, 2.0], [5.0, 2.2], [9.0, 1.8], [13.0, 2.0]]);
        let reference = path(&points, 50);
        for scale in [0.1, 1.0, 8.0] {
            for origin in [[100.0, 80.0], [900.0, 700.0]] {
                let transform = Transform {
                    scale,
                    origin,
                    center: [0.0; 2],
                };
                let recovered: Vec<_> = points
                    .iter()
                    .map(|p| {
                        InkPoint::mouse(transform.to_document(transform.to_screen(p.position)))
                    })
                    .collect();
                for (a, b) in path(&recovered, 50).iter().zip(&reference) {
                    assert!(distance(*a, *b) < 1e-9);
                }
            }
        }
        let distant: Vec<_> = points
            .iter()
            .map(|p| InkPoint::mouse(p.position.map(|v| v + 1e12)))
            .collect();
        for (a, b) in path(&distant, 50).iter().zip(&reference) {
            assert!(distance(a.map(|v| v - 1e12), *b) < 0.001);
        }
    }
    #[test]
    #[ignore = "manual release-mode CPU benchmark"]
    fn smoothing_performance() {
        for count in [100, 100_000] {
            let input: Vec<_> = (0..count)
                .map(|i| InkPoint::mouse([i as f64, (i as f64 * 0.1).sin() * 10.0]))
                .collect();
            let mut output = Vec::new();
            let mut preview = Preview::default();
            let start = std::time::Instant::now();
            for end in 1..=count {
                std::hint::black_box(preview.update(&input[..end], 50, &mut output));
            }
            eprintln!(
                "{count} samples, incremental smoothing: {:?}, {} path points",
                start.elapsed(),
                output.len()
            );
            let start = std::time::Instant::now();
            std::hint::black_box(path(&input, 50));
            eprintln!("{count} samples, one full rebuild: {:?}", start.elapsed());
        }
    }
}
