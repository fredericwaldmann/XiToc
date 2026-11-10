//! Portable editable text semantics. Layout, caret and IME state are adapters.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const MAX_TEXT_BYTES: usize = 65536;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFamily {
    Sans,
    Monospace,
    Installed(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    pub family: TextFamily,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: [u8; 4],
    pub alignment: Alignment,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: TextFamily::Sans,
            size: 18.,
            bold: false,
            italic: false,
            underline: false,
            color: [32, 39, 49, 255],
            alignment: Alignment::Left,
        }
    }
}
impl TextStyle {
    pub fn valid(&self) -> bool {
        self.size.is_finite() && (4.0..=512.).contains(&self.size)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextContent {
    pub text: String,
    pub style: TextStyle,
    #[serde(default)]
    pub runs: Vec<FormatRun>,
    #[serde(default)]
    pub paragraphs: Vec<ParagraphStyle>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextObject {
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    pub width: f64,
    pub height: f64,
    pub content: std::sync::Arc<TextContent>,
    #[serde(default)]
    #[serde(alias = "box_style")]
    pub layout: TextLayout,
}
impl TextObject {
    pub fn content_mut(&mut self) -> &mut TextContent {
        std::sync::Arc::make_mut(&mut self.content)
    }
    pub fn new(owner: Uuid, surface: Surface, point: Point, width: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                position: point,
                ..Default::default()
            },
            width,
            height: 26.,
            content: Default::default(),
            layout: Default::default(),
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && self.placement.reflected == [false; 2]
            && self
                .placement
                .position
                .iter()
                .chain(&self.placement.origin)
                .all(|v| v.abs() <= 1e15)
            && self.width.is_finite()
            && (24.0..=1e6).contains(&self.width)
            && self.height.is_finite()
            && (1.0..=1e7).contains(&self.height)
            && self.content.valid()
            && self.content.text.len() <= MAX_TEXT_BYTES
            && !self.content.text.contains('\0')
    }
    pub fn corners(&self) -> [Point; 4] {
        self.local_bounds()
            .unwrap()
            .corners()
            .map(|p| self.placement.apply(p))
    }
    /// Group scaling scales type uniformly. Reflection moves centers, retaining
    /// readable glyph orientation; explicit rotation rotates the text normally.
    pub fn transformed(&self, delta: Placement) -> Self {
        let mut out = self.clone();
        let center = delta.apply(self.placement.apply([self.width * 0.5, self.height * 0.5]));
        out.placement.scale *= delta.scale;
        out.placement.rotation += delta.rotation;
        out.placement.origin = [0.; 2];
        out.placement.position = [0.; 2];
        let v = out.placement.apply([out.width * 0.5, out.height * 0.5]);
        out.placement.position = std::array::from_fn(|i| center[i] - v[i]);
        out
    }
    pub fn rectangle_matches(&self, b: ObjectBounds, crossing: bool) -> bool {
        let mut p = self.corners().to_vec();
        p.push(p[0]);
        crate::selection::rectangle_matches(b, crossing, &p, &[], 0., Default::default())
            || (crossing
                && b.corners()
                    .iter()
                    .any(|p| crate::selection::inside(*p, &self.corners())))
    }
    pub fn lasso_matches(&self, region: &crate::selection::LassoRegion) -> bool {
        let mut p = self.corners().to_vec();
        p.push(p[0]);
        region.encloses(&p, &[], 0., Default::default())
    }
}
impl CanvasObject for TextObject {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Text
    }
    fn document_id(&self) -> Uuid {
        self.document_id
    }
    fn surface(&self) -> Surface {
        self.surface
    }
    fn placement(&self) -> Placement {
        self.placement
    }
    fn local_bounds(&self) -> Option<ObjectBounds> {
        Some(ObjectBounds {
            min: [0.; 2],
            max: [self.width, self.height],
        })
    }
    fn hit_test(&self, p: Point, tolerance: f64) -> bool {
        if !self.valid() {
            return false;
        }
        let p = self.placement.inverse(p);
        let t = tolerance / self.placement.scale;
        p[0] >= -t && p[1] >= -t && p[0] <= self.width + t && p[1] <= self.height + t
    }
}

#[derive(Default, PartialEq)]
pub struct TextStylePatch {
    pub family: Option<TextFamily>,
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub rgb: Option<[u8; 3]>,
    pub alpha: Option<u8>,
    pub alignment: Option<Alignment>,
}
impl TextStylePatch {
    pub fn apply(&self, style: &mut TextStyle) {
        if let Some(v) = &self.family {
            style.family = v.clone();
        }
        if let Some(v) = self.size {
            style.size = v;
        }
        if let Some(v) = self.bold {
            style.bold = v;
        }
        if let Some(v) = self.italic {
            style.italic = v;
        }
        if let Some(v) = self.underline {
            style.underline = v;
        }
        if let Some(v) = self.rgb {
            style.color[..3].copy_from_slice(&v);
        }
        if let Some(v) = self.alpha {
            style.color[3] = v;
        }
        if let Some(v) = self.alignment {
            style.alignment = v;
        }
    }
}

/// Unicode scalar offsets, matching the editor protocol. Public operations snap
/// ranges outward to extended grapheme boundaries before changing content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatRun {
    pub range: std::ops::Range<usize>,
    pub style: TextStyle,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParagraphStyle {
    pub alignment: Alignment,
    pub line_spacing: f64,
    pub spacing: f64,
}
impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            alignment: Alignment::Left,
            line_spacing: 1.,
            spacing: 0.,
        }
    }
}
impl TextContent {
    /// Scale every stored character size without changing any other character
    /// or paragraph property. A reciprocal factor can undo an unclamped scale.
    pub fn scale_font_sizes(&mut self, factor: f64) -> bool {
        if !factor.is_finite() || factor <= 0.0 {
            return false;
        }
        let mut changed = false;
        let scale = |size: &mut f64, changed: &mut bool| {
            let next = (*size * factor).clamp(4.0, 512.0);
            if next != *size {
                *size = next;
                *changed = true;
            }
        };
        scale(&mut self.style.size, &mut changed);
        for run in &mut self.runs {
            scale(&mut run.style.size, &mut changed);
        }
        changed
    }

    pub fn valid(&self) -> bool {
        let n = self.text.chars().count();
        self.style.valid()
            && self.runs.iter().enumerate().all(|(i, r)| {
                r.range.start < r.range.end
                    && r.range.end <= n
                    && r.style.valid()
                    && (i == 0 || self.runs[i - 1].range.end <= r.range.start)
            })
            && self.paragraphs.iter().all(|p| {
                p.line_spacing.is_finite()
                    && (0.5..=4.).contains(&p.line_spacing)
                    && p.spacing.is_finite()
                    && (0.0..=512.).contains(&p.spacing)
            })
    }
    pub fn style_at(&self, index: usize) -> &TextStyle {
        let i = self.runs.partition_point(|r| r.range.end <= index);
        self.runs
            .get(i)
            .filter(|r| r.range.contains(&index))
            .map(|r| &r.style)
            .unwrap_or(&self.style)
    }
    pub fn segments(&self) -> Vec<FormatRun> {
        let mut out = Vec::new();
        let mut start = 0;
        for run in &self.runs {
            if start < run.range.start {
                out.push(FormatRun {
                    range: start..run.range.start,
                    style: self.style.clone(),
                });
            }
            out.push(run.clone());
            start = run.range.end;
        }
        let n = self.text.chars().count();
        if start < n {
            out.push(FormatRun {
                range: start..n,
                style: self.style.clone(),
            });
        }
        out
    }
    pub fn grapheme_range(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        use unicode_segmentation::UnicodeSegmentation;
        let mut boundaries = vec![0];
        for g in self.text.graphemes(true) {
            boundaries.push(boundaries.last().copied().unwrap() + g.chars().count());
        }
        let n = *boundaries.last().unwrap();
        let start = *boundaries
            .iter()
            .rev()
            .find(|i| **i <= range.start.min(n))
            .unwrap();
        let end = *boundaries.iter().find(|i| **i >= range.end.min(n)).unwrap();
        if range.is_empty() {
            start..start
        } else {
            start..end
        }
    }
    fn styles(&self) -> Vec<TextStyle> {
        let mut styles = vec![self.style.clone(); self.text.chars().count()];
        for run in &self.runs {
            for style in &mut styles[run.range.clone()] {
                *style = run.style.clone();
            }
        }
        styles
    }
    fn set_styles(&mut self, mut styles: Vec<TextStyle>) {
        use unicode_segmentation::UnicodeSegmentation;
        let mut offset = 0;
        for g in self.text.graphemes(true) {
            let count = g.chars().count();
            let style = styles[offset].clone();
            for s in &mut styles[offset..offset + count] {
                *s = style.clone();
            }
            offset += count;
        }
        self.runs.clear();
        for (i, style) in styles.into_iter().enumerate() {
            if let Some(last) = self.runs.last_mut()
                && last.style == style
            {
                last.range.end = i + 1;
                continue;
            }
            self.runs.push(FormatRun {
                range: i..i + 1,
                style,
            });
        }
    }
    pub fn format(&mut self, range: std::ops::Range<usize>, patch: &TextStylePatch) {
        let range = self.grapheme_range(range);
        if range.is_empty() {
            return;
        }
        let mut styles = self.styles();
        for style in &mut styles[range] {
            patch.apply(style);
        }
        self.set_styles(styles);
    }
    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        let range = self.grapheme_range(range);
        let mut out = Self {
            text: self
                .text
                .chars()
                .skip(range.start)
                .take(range.len())
                .collect(),
            style: self.style.clone(),
            ..Default::default()
        };
        out.runs = self
            .runs
            .iter()
            .filter_map(|run| {
                let start = run.range.start.max(range.start);
                let end = run.range.end.min(range.end);
                (start < end).then(|| FormatRun {
                    range: start - range.start..end - range.start,
                    style: run.style.clone(),
                })
            })
            .collect();
        let first = self
            .text
            .chars()
            .take(range.start)
            .filter(|c| *c == '\n')
            .count();
        out.paragraphs = (first..=first + out.text.matches('\n').count())
            .map(|i| self.paragraph(i))
            .collect();
        out
    }
    pub fn paragraph(&self, index: usize) -> ParagraphStyle {
        self.paragraphs
            .get(index)
            .cloned()
            .unwrap_or(ParagraphStyle {
                alignment: self.style.alignment,
                ..Default::default()
            })
    }
    pub fn format_paragraphs(
        &mut self,
        range: std::ops::Range<usize>,
        patch: impl Fn(&mut ParagraphStyle),
    ) {
        let first = self
            .text
            .chars()
            .take(range.start)
            .filter(|c| *c == '\n')
            .count();
        let last = self
            .text
            .chars()
            .take(range.end.max(range.start + 1).saturating_sub(1))
            .filter(|c| *c == '\n')
            .count();
        self.paragraphs = (0..=self.text.matches('\n').count())
            .map(|i| self.paragraph(i))
            .collect();
        for p in &mut self.paragraphs[first..=last] {
            patch(p);
        }
    }
    pub fn replace(&mut self, range: std::ops::Range<usize>, value: &TextContent) {
        let range = self.grapheme_range(range);
        let mut styles = self.styles();
        styles.splice(range.clone(), value.styles());
        let first = self
            .text
            .chars()
            .take(range.start)
            .filter(|c| *c == '\n')
            .count();
        let removed = self
            .text
            .chars()
            .skip(range.start)
            .take(range.len())
            .filter(|c| *c == '\n')
            .count();
        let inherited = self.paragraph(first);
        let mut paragraphs: Vec<_> = (0..=self.text.matches('\n').count())
            .map(|i| self.paragraph(i))
            .collect();
        // A complete rich replacement adopts its first paragraph. Partial
        // insertion keeps the destination paragraph, as ordinary typing does.
        if range.start == 0
            && range.end == self.text.chars().count()
            && let Some(first) = value.paragraphs.first()
        {
            paragraphs[0] = first.clone();
        }
        paragraphs.splice(
            first + 1..first + 1 + removed,
            (0..value.text.matches('\n').count()).map(|i| {
                value
                    .paragraphs
                    .get(i + 1)
                    .cloned()
                    .unwrap_or(inherited.clone())
            }),
        );
        let byte = |i| {
            self.text
                .char_indices()
                .nth(i)
                .map(|(b, _)| b)
                .unwrap_or(self.text.len())
        };
        self.text
            .replace_range(byte(range.start)..byte(range.end), &value.text);
        self.paragraphs = paragraphs;
        self.set_styles(styles);
    }
    pub fn scale_sizes(&mut self, factor: f64) {
        self.style.size = (self.style.size * factor).clamp(4., 512.);
        for run in &mut self.runs {
            run.style.size = (run.style.size * factor).clamp(4., 512.);
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResizeBehavior {
    #[default]
    Reflow,
    Scale,
}
/// Standalone text has invisible layout bounds, never a decorative container.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TextLayout {
    pub resize: ResizeBehavior,
}
impl<'de> Deserialize<'de> for TextLayout {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Accept the former session/clipboard box_style shape. Only resize is
        // retained; known decorative properties are consumed without rendering
        // or re-serializing them. Future unfamiliar fields still fail explicitly.
        #[derive(Default, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        struct CompatibleLayout {
            resize: ResizeBehavior,
            #[serde(rename = "background")]
            _background: Option<serde::de::IgnoredAny>,
            #[serde(rename = "border")]
            _border: Option<serde::de::IgnoredAny>,
            #[serde(rename = "padding")]
            _padding: Option<serde::de::IgnoredAny>,
            #[serde(rename = "radius")]
            _radius: Option<serde::de::IgnoredAny>,
            #[serde(rename = "opacity")]
            _opacity: Option<serde::de::IgnoredAny>,
        }
        let value = CompatibleLayout::deserialize(deserializer)?;
        Ok(Self {
            resize: value.resize,
        })
    }
}

/// Local normalized handles, ordered corners then edges. Same captured source
/// and pointer-start displacement as the common transform gesture protocol.
pub const TEXT_HANDLES: [[f64; 2]; 8] = [
    [0., 0.],
    [1., 0.],
    [0., 1.],
    [1., 1.],
    [0., 0.5],
    [1., 0.5],
    [0.5, 0.],
    [0.5, 1.],
];
impl TextObject {
    pub fn resized(&self, handle: usize, start: Point, pointer: Point) -> Self {
        let h = TEXT_HANDLES[handle];
        let a = self.placement.inverse(start);
        let b = self.placement.inverse(pointer);
        let delta = [b[0] - a[0], b[1] - a[1]];
        let mut out = self.clone();
        let mut width = if h[0] == 0.5 {
            self.width
        } else {
            self.width + delta[0] * if h[0] == 0. { -1. } else { 1. }
        };
        let mut height = if h[1] == 0.5 {
            self.height
        } else {
            self.height + delta[1] * if h[1] == 0. { -1. } else { 1. }
        };
        if self.layout.resize == ResizeBehavior::Scale && handle < 4 {
            let diagonal = [
                self.width * (2. * h[0] - 1.),
                self.height * (2. * h[1] - 1.),
            ];
            let sizes = std::iter::once(self.content.style.size)
                .chain(self.content.runs.iter().map(|r| r.style.size));
            let (min, max) = sizes.fold((512.0_f64, 4.0_f64), |(min, max), v| {
                (min.min(v), max.max(v))
            });
            let lower = (24. / self.width).max(4. / min);
            let upper = (1e6 / self.width).min(512. / max);
            let factor = (1.
                + (delta[0] * diagonal[0] + delta[1] * diagonal[1])
                    / (self.width * self.width + self.height * self.height))
                .clamp(lower, upper);
            width = self.width * factor;
            height = self.height * factor;
            out.content_mut().scale_sizes(factor);
        }
        out.width = width.clamp(24., 1e6);
        out.height = height.clamp(1., 1e7);
        let shift = [
            if h[0] == 0. {
                self.width - out.width
            } else {
                0.
            },
            if h[1] == 0. {
                self.height - out.height
            } else {
                0.
            },
        ];
        out.placement.position = self.placement.apply(shift);
        // Keep origin semantics even for objects transformed by group operations.
        let zero = self.placement.apply([0., 0.]);
        out.placement.position = std::array::from_fn(|i| {
            self.placement.position[i] + out.placement.position[i] - zero[i]
        });
        out
    }
}

impl TextObject {
    pub fn anchor_resize(&mut self, source: &Self, handle: usize) {
        let h = TEXT_HANDLES[handle];
        let a = [
            if h[0] == 0. { 1. } else { 0. },
            if h[1] == 0. { 1. } else { 0. },
        ];
        let before = source
            .placement
            .apply([source.width * a[0], source.height * a[1]]);
        let after = self
            .placement
            .apply([self.width * a[0], self.height * a[1]]);
        for i in 0..2 {
            self.placement.position[i] += before[i] - after[i];
        }
    }
}
