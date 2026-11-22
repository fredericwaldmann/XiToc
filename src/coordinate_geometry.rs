//! Derived vector geometry in logical object coordinates.
use crate::{
    coordinate::{Arrow, Config, Pattern},
    coordinate_expression,
    object::Point,
};

#[derive(Clone, Copy)]
pub enum Color {
    Axis,
    Tick,
    Custom([u8; 3], u8),
}
pub struct Line {
    pub points: Vec<Point>,
    pub width: f64,
    pub color: Color,
    pub pattern: Pattern,
}
pub struct Label {
    pub at: Point,
    pub text: String,
    pub size: f32,
    pub math: bool,
}
pub struct Dot {
    pub at: Point,
    pub color: Color,
    pub closed: bool,
}
#[derive(Default)]
pub struct Geometry {
    pub lines: Vec<Line>,
    pub arrows: Vec<[Point; 3]>,
    pub labels: Vec<Label>,
    pub dots: Vec<Dot>,
}

struct Mapping {
    basis: [[f64; 2]; 3],
    scale: Point,
    origin: Point,
}
impl Mapping {
    fn new(config: &Config, size: Point) -> Self {
        let count = config.preset.dimension();
        let basis = if count == 3 {
            let (sa, ca) = config.azimuth.to_radians().sin_cos();
            let (se, ce) = config.elevation.to_radians().sin_cos();
            [[ca, sa * se], [-sa, ca * se], [0., -ce]]
        } else {
            [[1., 0.], [0., -1.], [0., 0.]]
        };
        let mut min = [0_f64; 2];
        let mut max = [0_f64; 2];
        for (axis, vector) in config.axes.iter().zip(basis).take(count) {
            for j in 0..2 {
                let a = axis.range[0] * vector[j];
                let b = axis.range[1] * vector[j];
                min[j] += a.min(b);
                max[j] += a.max(b);
            }
        }
        let padding = (size[0].min(size[1]) * 0.21).min(30.);
        let mut scale: Point =
            std::array::from_fn(|i| (size[i] - 2. * padding) / (max[i] - min[i]).max(1e-12));
        if count == 1 {
            scale[1] = scale[0];
        } else if count == 3 || config.equal_units {
            scale = [scale[0].min(scale[1]); 2];
        }
        let origin = std::array::from_fn(|i| size[i] / 2. - (min[i] + max[i]) * scale[i] / 2.);
        Self {
            basis,
            scale,
            origin,
        }
    }
    fn point(&self, point: [f64; 3]) -> Point {
        std::array::from_fn(|i| {
            self.origin[i]
                + (0..3).map(|a| self.basis[a][i] * point[a]).sum::<f64>() * self.scale[i]
        })
    }
}
fn axis_point(axis: usize, value: f64) -> [f64; 3] {
    let mut p = [0.; 3];
    p[axis] = value;
    p
}
fn offset(p: Point, v: Point, amount: f64) -> Point {
    [p[0] + v[0] * amount, p[1] + v[1] * amount]
}
fn normalized(a: Point, b: Point) -> Point {
    let v = [b[0] - a[0], b[1] - a[1]];
    let length = v[0].hypot(v[1]).max(1e-12);
    [v[0] / length, v[1] / length]
}
fn ticks(range: Point, spacing: f64) -> impl Iterator<Item = (i64, f64)> {
    let first = (range[0] / spacing).ceil() as i64;
    let last = (range[1] / spacing).floor() as i64;
    let stride = (last - first).max(0) / 126 + 1;
    let aligned = first.div_euclid(stride) * stride;
    (0_i64..=127)
        .map(move |i| aligned + i * stride)
        .filter(move |&n| n >= first && n <= last && n != 0)
        .map(move |n| (n, n as f64 * spacing))
}
fn number(value: f64) -> String {
    if value.abs() < 0.001 || value.abs() >= 10000. {
        format!("{value:.2e}")
    } else {
        format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}
impl Geometry {
    fn segment(&mut self, a: Point, b: Point, width: f64, color: Color) {
        self.lines.push(Line {
            points: vec![a, b],
            width,
            color,
            pattern: Pattern::Solid,
        });
    }
    fn arrow(&mut self, tip: Point, from: Point, config: &Config) {
        let v = normalized(from, tip);
        let n = [-v[1], v[0]];
        let length = 7. + config.axis_width;
        let base = offset(tip, v, -length);
        let arrow = [
            offset(base, n, length * 0.36),
            tip,
            offset(base, n, -length * 0.36),
        ];
        match config.arrow {
            Arrow::None => (),
            Arrow::Scientific => self.arrows.push(arrow),
            Arrow::Open => self.lines.push(Line {
                points: arrow.to_vec(),
                width: config.axis_width,
                color: Color::Axis,
                pattern: Pattern::Solid,
            }),
        }
    }
}

pub fn generate(
    config: &Config,
    size: Point,
    quality: usize,
    samples: &mut SamplingCache,
) -> Geometry {
    let mut result = Geometry::default();
    let map = Mapping::new(config, size);
    let count = config.preset.dimension();
    // The origin label is attached to one visible axis tick position (X when
    // available), never emitted as an unrelated floating annotation.
    let zero_axis = config
        .axes
        .iter()
        .enumerate()
        .take(count)
        .find(|(_, axis)| axis.visible && axis.range[0] <= 0. && axis.range[1] >= 0.)
        .map(|(i, _)| i);
    if config.grid && count >= 2 {
        for i in 0..2 {
            let other = 1 - i;
            for (n, value) in ticks(
                config.axes[i].range,
                config.grid_step / f64::from(config.grid_subdivisions),
            ) {
                let mut a = axis_point(i, value);
                let mut b = a;
                [a[other], b[other]] = config.axes[other].range;
                let major = n % i64::from(config.grid_subdivisions) == 0;
                result.segment(
                    map.point(a),
                    map.point(b),
                    config.grid_width * if major { 1. } else { 0.65 },
                    Color::Custom(
                        config.grid_color,
                        if major {
                            config.grid_opacity
                        } else {
                            config.grid_opacity / 2
                        },
                    ),
                );
            }
        }
    }
    if count == 1 {
        let range = config.axes[0].range;
        for item in &config.markers {
            let color = Color::Custom(item.color, 255);
            if let Some(end) = item.end {
                let a = item.start.max(range[0]);
                let b = end.min(range[1]);
                if a <= b {
                    let at = |v| offset(map.point([v, 0., 0.]), [0., -1.], 10.);
                    let mut start = at(a);
                    let mut finish = at(b);
                    if a == item.start && !item.closed[0] {
                        start[0] += 3.7;
                    }
                    if b == end && !item.closed[1] {
                        finish[0] -= 3.7;
                    }
                    if start[0] < finish[0] {
                        result.segment(start, finish, 2., color);
                    }
                    for (value, closed) in [(item.start, item.closed[0]), (end, item.closed[1])] {
                        if (range[0]..=range[1]).contains(&value) {
                            result.dots.push(Dot {
                                at: at(value),
                                color,
                                closed,
                            });
                        }
                    }
                }
            } else if (range[0]..=range[1]).contains(&item.start) {
                result.dots.push(Dot {
                    at: offset(map.point([item.start, 0., 0.]), [0., -1.], 10.),
                    color,
                    closed: item.closed[0],
                });
            }
        }
    } else {
        let plots = if count == 2 {
            &config.curves
        } else {
            &config.surfaces
        };
        for (index, plot) in plots.iter().enumerate().filter(|(_, plot)| plot.visible) {
            let runs = samples.runs(config, plot, index, quality);
            result.lines.extend(runs.iter().map(|run| Line {
                points: run.iter().map(|&point| map.point(point)).collect(),
                width: plot.width,
                color: Color::Custom(plot.color, 255),
                pattern: plot.pattern,
            }));
        }
    }
    for (i, axis) in config
        .axes
        .iter()
        .enumerate()
        .take(count)
        .filter(|(_, axis)| axis.visible)
    {
        let [low, high] = axis.range;
        let a = map.point(axis_point(i, low));
        let b = map.point(axis_point(i, high));
        let direction = normalized(a, b);
        let normal = if i == 0 {
            [-direction[1], direction[0]]
        } else {
            [direction[1], -direction[0]]
        };
        result.segment(a, b, config.axis_width, Color::Axis);
        result.arrow(b, a, config);
        if low < 0. {
            result.arrow(a, b, config);
        }
        if axis.show_label && !axis.label.is_empty() {
            result.labels.push(Label {
                at: offset(b, direction, 13.),
                text: axis.label.clone(),
                size: 14.,
                math: true,
            });
        }
        if config.ticks || config.numbers {
            for (_, value) in ticks(axis.range, config.tick_step) {
                // Leave arrowheads and axis labels room near the tips.
                let at = map.point(axis_point(i, value));
                if [a, b]
                    .into_iter()
                    .any(|p| (p[0] - at[0]).hypot(p[1] - at[1]) < 9.)
                {
                    continue;
                }
                if config.ticks {
                    result.segment(
                        offset(at, normal, -config.tick_length / 2.),
                        offset(at, normal, config.tick_length / 2.),
                        config.tick_width,
                        Color::Tick,
                    );
                }
                if config.numbers {
                    result.labels.push(Label {
                        at: offset(at, normal, config.tick_length / 2. + 9.),
                        text: number(value),
                        size: 10.,
                        math: true,
                    });
                }
            }
        }
        if config.show_zero && zero_axis == Some(i) && !config.origin_label.is_empty() {
            // This is the native equivalent of an xtick/xticklabels entry.
            // It follows the selected axis basis and remains independent of
            // ordinary tick-mark visibility.
            let at = map.point(axis_point(i, 0.));
            result.labels.push(Label {
                at: offset(at, normal, config.tick_length / 2. + 9.),
                text: config.origin_label.clone(),
                size: 12.,
                math: true,
            });
        }
    }
    result
}
fn intersect(range: Point, domain: Option<Point>) -> Option<Point> {
    let domain = domain.unwrap_or(range);
    let result = [range[0].max(domain[0]), range[1].min(domain[1])];
    (result[0] < result[1]).then_some(result)
}

#[derive(Clone, Copy)]
struct Sampler {
    output: Point,
    steps: usize,
    depth: u8,
    relative_error: f64,
}

fn sample(
    settings: Sampler,
    domain: Point,
    evaluate: impl Fn(f64) -> [f64; 3],
    ordinate: usize,
) -> Vec<Vec<[f64; 3]>> {
    let mut runs = Vec::new();
    let mut current = Vec::new();
    let mut flush = |current: &mut Vec<_>| {
        if !current.is_empty() {
            runs.push(std::mem::take(current));
        }
    };
    let mut emit = |segment: Option<[[f64; 3]; 2]>| {
        if let Some([a, b]) = segment {
            if current.last() != Some(&a) {
                flush(&mut current);
                current.push(a);
            }
            current.push(b);
        } else {
            flush(&mut current);
        }
    };
    let mut left = (domain[0], evaluate(domain[0]));
    for index in 1..=settings.steps {
        let t = domain[0] + (domain[1] - domain[0]) * index as f64 / settings.steps as f64;
        let right = (t, evaluate(t));
        adaptive(
            settings,
            ordinate,
            &evaluate,
            left,
            right,
            settings.depth,
            &mut emit,
        );
        left = right;
    }
    flush(&mut current);
    runs
}

fn adaptive(
    settings: Sampler,
    axis: usize,
    evaluate: &impl Fn(f64) -> [f64; 3],
    a: (f64, [f64; 3]),
    b: (f64, [f64; 3]),
    depth: u8,
    emit: &mut impl FnMut(Option<[[f64; 3]; 2]>),
) {
    let middle = (a.0 + b.0) * 0.5;
    let m = evaluate(middle);
    let [v, w] = [a.1[axis], b.1[axis]];
    let z = m[axis];
    let span = settings.output[1] - settings.output[0];
    let finite = [v, z, w].into_iter().all(f64::is_finite);
    if !v.is_finite() && !z.is_finite() && !w.is_finite() {
        emit(None);
        return;
    }
    let curved = !finite
        || (z - v * 0.5 - w * 0.5).abs() > span * settings.relative_error
        || (v - w).abs() > span * 0.35;
    if curved && depth > 0 {
        adaptive(settings, axis, evaluate, a, (middle, m), depth - 1, emit);
        adaptive(settings, axis, evaluate, (middle, m), b, depth - 1, emit);
    } else if !finite || curved && (z - v * 0.5 - w * 0.5).abs() > span * 0.04 {
        emit(None);
    } else {
        emit(clip(a.1, b.1, axis, settings.output));
    }
}
fn clip(a: [f64; 3], b: [f64; 3], axis: usize, [low, high]: Point) -> Option<[[f64; 3]; 2]> {
    let [v, w] = [a[axis], b[axis]];
    if !v.is_finite() || !w.is_finite() || (v < low && w < low) || (v > high && w > high) {
        return None;
    }
    let point = |p: [f64; 3], q: [f64; 3]| {
        let target = p[axis].clamp(low, high);
        if p[axis] == target {
            return p;
        }
        let magnitude = p[axis].abs().max(q[axis].abs()).max(1.);
        let t = ((target / magnitude) - (p[axis] / magnitude))
            / ((q[axis] / magnitude) - (p[axis] / magnitude));
        let mut output = std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t);
        output[axis] = target;
        output
    };
    Some([point(a, b), point(b, a)])
}

#[derive(Default)]
pub struct SamplingCache {
    curves: Vec<Option<SampledPlot>>,
    surfaces: Vec<Option<SampledPlot>>,
    #[cfg(test)]
    recomputations: usize,
}

#[derive(PartialEq)]
struct SampleKey {
    expression: String,
    ranges: [Point; 3],
    x_domain: Option<Point>,
    y_domain: Option<Point>,
    quality: usize,
}
struct SampledPlot {
    key: SampleKey,
    runs: Vec<Vec<[f64; 3]>>,
}

impl SamplingCache {
    fn runs(
        &mut self,
        config: &Config,
        plot: &crate::coordinate::Plot,
        index: usize,
        quality: usize,
    ) -> &[Vec<[f64; 3]>] {
        let surface = config.preset.dimension() == 3;
        let mut ranges = std::array::from_fn(|i| config.axes[i].range);
        if !surface {
            ranges[2] = [0.; 2];
        }
        let key = SampleKey {
            expression: plot.expression.clone(),
            ranges,
            x_domain: plot.domain,
            y_domain: if surface { plot.y_domain } else { None },
            quality,
        };
        let slots = if surface {
            &mut self.surfaces
        } else {
            &mut self.curves
        };
        if slots.len() <= index {
            slots.resize_with(index + 1, || None);
        }
        let slot = &mut slots[index];
        if slot.as_ref().is_none_or(|previous| previous.key != key) {
            let runs = sample_plot(&key, surface);
            *slot = Some(SampledPlot { key, runs });
            #[cfg(test)]
            {
                self.recomputations += 1;
            }
        }
        &slot.as_ref().unwrap().runs
    }
}

fn sample_plot(key: &SampleKey, surface: bool) -> Vec<Vec<[f64; 3]>> {
    let Ok(expr) = coordinate_expression::parse(&key.expression, surface) else {
        return vec![];
    };
    let Some(x) = intersect(key.ranges[0], key.x_domain) else {
        return vec![];
    };
    if !surface {
        return sample(
            Sampler {
                output: key.ranges[1],
                steps: ((key.quality as f64).sqrt() * 4.).min(192.) as usize,
                depth: 5,
                relative_error: 0.25 / key.quality.max(1) as f64,
            },
            x,
            |t| [t, expr.evaluate(t, 0.), 0.],
            1,
        );
    }
    let Some(y) = intersect(key.ranges[1], key.y_domain) else {
        return vec![];
    };
    let settings = Sampler {
        output: key.ranges[2],
        steps: 24,
        depth: 4,
        relative_error: 0.25 / key.quality.max(1) as f64,
    };
    let mut runs = Vec::new();
    for i in 0..=12 {
        let t = f64::from(i) / 12.;
        let u = x[0] + (x[1] - x[0]) * t;
        let v = y[0] + (y[1] - y[0]) * t;
        runs.extend(sample(settings, x, |x| [x, v, expr.evaluate(x, v)], 2));
        runs.extend(sample(settings, y, |y| [u, y, expr.evaluate(u, y)], 2));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate::{Plot, Preset};
    #[test]
    fn physical_plot_extent_controls_approximation_error() {
        for expression in ["x^2", "sin(x)"] {
            let evaluate = coordinate_expression::parse(expression, false).unwrap();
            let key = |quality| SampleKey {
                expression: expression.into(),
                ranges: [[-3., 3.], [-1., 10.], [0.; 2]],
                x_domain: None,
                y_domain: None,
                quality,
            };
            let small = sample_plot(&key(256), false);
            let large = sample_plot(&key(4096), false);
            assert!(large.iter().map(Vec::len).sum::<usize>() > small.iter().map(Vec::len).sum());
            for pair in large.iter().flat_map(|run| run.windows(2)) {
                let (a, b) = (pair[0], pair[1]);
                let midpoint = (a[0] + b[0]) * 0.5;
                let error = (evaluate.evaluate(midpoint, 0.) - (a[1] + b[1]) * 0.5).abs();
                assert!(error * 4096. / 11. <= 0.25);
            }
        }
    }

    #[test]
    fn five_presets_project_axes_and_label_zero_once() {
        for (preset, arrows) in Preset::ALL.into_iter().zip([1, 2, 2, 4, 3]) {
            let config = Config::new(preset);
            assert!(config.validate().is_ok());
            let geometry = generate(&config, [330., 300.], 96, &mut SamplingCache::default());
            assert_eq!(geometry.arrows.len(), arrows);
            assert_eq!(
                geometry
                    .labels
                    .iter()
                    .filter(|label| label.text == "0")
                    .count(),
                1
            );
            assert_eq!(geometry.labels.len(), preset.dimension() + 1);
            assert!(geometry.labels.iter().all(|label| label.math));
        }
        let mut config = Config::new(Preset::FourQuadrants);
        config.axes[0].range = [-2., 9.];
        let map = Mapping::new(&config, [400., 180.]);
        assert_eq!(map.scale[0], map.scale[1]);
        let a = map.point([0.; 3]);
        let b = map.point([1., 1., 0.]);
        assert!(((b[0] - a[0]) + (b[1] - a[1])).abs() < 1e-10);
    }
    #[test]
    fn numeric_ticks_and_zero_label_are_independent_for_every_combination() {
        for preset in Preset::ALL {
            for (numbers, show_zero, expected_zero) in [
                (false, true, 1),
                (false, false, 0),
                (true, true, 1),
                (true, false, 0),
            ] {
                let mut config = Config::new(preset);
                if preset == Preset::FourQuadrants {
                    config.axes[0].range = [-3., 3.];
                    config.axes[1].range = [-3., 3.];
                }
                config.numbers = numbers;
                config.show_zero = show_zero;
                config.ticks = false;
                let geometry = generate(&config, [330., 300.], 96, &mut SamplingCache::default());
                let zero_count = geometry
                    .labels
                    .iter()
                    .filter(|label| label.text == "0")
                    .count();
                assert_eq!(zero_count, expected_zero, "{preset:?}");
                let numeric_count = geometry
                    .labels
                    .iter()
                    .filter(|label| label.text.parse::<f64>().is_ok())
                    .count();
                assert_eq!(numeric_count > expected_zero, numbers, "{preset:?}");
                assert!(geometry.labels.iter().all(|label| label.math));
            }
        }
    }
    #[test]
    fn zero_label_uses_the_x_axis_tick_anchor_and_stays_off_when_ticks_are_hidden() {
        let mut config = Config::new(Preset::FourQuadrants);
        config.ticks = false;
        let size = [400., 300.];
        let geometry = generate(&config, size, 96, &mut SamplingCache::default());
        let zero = geometry
            .labels
            .iter()
            .find(|label| label.text == "0")
            .unwrap();
        let origin = Mapping::new(&config, size).point([0.; 3]);
        assert!((zero.at[0] - origin[0]).abs() < 1e-9);
        assert!((zero.at[1] - origin[1] - (config.tick_length / 2. + 9.)).abs() < 1e-9);
        config.show_zero = false;
        let hidden = generate(&config, size, 96, &mut SamplingCache::default());
        assert!(!hidden.labels.iter().any(|label| label.text == "0"));
    }
    #[test]
    fn grid_and_tick_sampling_stays_bounded_and_mathematically_aligned() {
        assert_eq!(
            ticks([-2., 3.], 1.).map(|(_, x)| x).collect::<Vec<_>>(),
            [-2., -1., 1., 2., 3.]
        );
        let dense = ticks([-1e6, 1e6], 1e-6).collect::<Vec<_>>();
        assert!(dense.len() <= 127);
        assert!(
            dense
                .iter()
                .all(|&(n, x)| x == n as f64 * 1e-6 && (-1e6..=1e6).contains(&x))
        );
        let mut config = Config::new(Preset::FirstQuadrant);
        config.grid = true;
        config.ticks = true;
        let geometry = generate(&config, [360., 300.], 96, &mut SamplingCache::default());
        assert_eq!(geometry.labels.len(), 3);
        assert!(geometry.lines.len() > 10);
    }

    #[test]
    fn clips_poles_and_undefined_domains_even_between_samples() {
        for (text, pole) in [
            ("1/x", 0.),
            ("1/(x-0.03)", 0.03),
            ("tan(x)", std::f64::consts::FRAC_PI_2),
        ] {
            let expr = coordinate_expression::parse(text, false).unwrap();
            let runs = sample(
                Sampler {
                    output: [-5., 5.],
                    steps: 79,
                    depth: 5,
                    relative_error: 0.002,
                },
                [-3., 3.],
                |x| [x, expr.evaluate(x, 0.), 0.],
                1,
            );
            assert!(runs.len() >= 2);
            for run in runs {
                assert!(run.iter().all(|p| (-5. ..=5.).contains(&p[1])));
                assert!(
                    !run.windows(2).any(|p| p[0][0] < pole && p[1][0] > pole),
                    "{text}"
                );
            }
        }
        let runs = sample(
            Sampler {
                output: [0., 2.],
                steps: 80,
                depth: 5,
                relative_error: 0.002,
            },
            [-3., 3.],
            |x| [x, x.sqrt(), 0.],
            1,
        );
        assert!(runs.iter().flatten().all(|p| p[0] >= 0.));
    }
    #[test]
    fn surface_view_changes_without_changing_expression_and_markers_survive_switch() {
        let mut config = Config::new(Preset::Positive);
        config.markers.push(crate::coordinate::Marker {
            start: 1.,
            end: Some(3.),
            closed: [true, false],
            color: [30, 50, 180],
        });
        let geometry = generate(&config, [330., 300.], 96, &mut SamplingCache::default());
        assert_eq!(
            geometry.dots.iter().map(|d| d.closed).collect::<Vec<_>>(),
            [true, false]
        );
        config.set_preset(Preset::Octant);
        config.surfaces.push(Plot::new(0, true));
        let mut samples = SamplingCache::default();
        let a = generate(&config, [330., 300.], 96, &mut samples);
        assert!(a.lines.len() > 5);
        assert!(
            a.lines
                .iter()
                .flat_map(|l| &l.points)
                .flatten()
                .all(|p| p.is_finite())
        );
        config.azimuth = 65.;
        let b = generate(&config, [330., 300.], 96, &mut samples);
        assert_ne!(a.lines[0].points, b.lines[0].points);
        assert_eq!(config.markers.len(), 1);
        assert_eq!(samples.recomputations, 1);
        config.surfaces[0].color = [255, 0, 0];
        let _ = generate(&config, [400., 220.], 96, &mut samples);
        assert_eq!(samples.recomputations, 1);
        config.surfaces[0].expression = "sin(x)*cos(y)".into();
        let _ = generate(&config, [400., 220.], 96, &mut samples);
        assert_eq!(samples.recomputations, 2);
    }
}
