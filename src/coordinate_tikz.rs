//! A bounded TikZ-inspired source format for coordinate diagrams.
//!
//! This deliberately implements a small, safe subset. It never invokes TeX.
use crate::coordinate::{Arrow, Config, Pattern, Plot, Preset};

pub const MAX_SOURCE_BYTES: usize = 32 * 1024;

/// Produce editable TikZ source for the supported diagram model. The `xitoc`
/// comments retain settings that ordinary TikZ has no direct representation
/// for, such as the native 3D view and 1D interval endpoints.
pub fn generate(config: &Config) -> String {
    generate_preserving(config, "")
}

pub fn generate_preserving(config: &Config, old: &str) -> String {
    let mut out = "\\begin{tikzpicture}\n".to_owned();
    for comment in old
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('%') && !line.starts_with("% xitoc "))
    {
        out.push_str(comment);
        out.push('\n');
    }
    for (key, value) in [
        ("preset", preset_name(config.preset).to_string()),
        ("azimuth", config.azimuth.to_string()),
        ("elevation", config.elevation.to_string()),
        ("equal-units", config.equal_units.to_string()),
        ("numbers", config.numbers.to_string()),
        ("origin-label", config.origin_label.replace(' ', "_")),
        ("ticks", config.ticks.to_string()),
        ("tick-step", config.tick_step.to_string()),
        ("tick-length", config.tick_length.to_string()),
        ("tick-width", config.tick_width.to_string()),
        ("grid", config.grid.to_string()),
        ("grid-step", config.grid_step.to_string()),
        ("grid-width", config.grid_width.to_string()),
        ("grid-opacity", config.grid_opacity.to_string()),
        ("grid-subdivisions", config.grid_subdivisions.to_string()),
        ("axis-width", config.axis_width.to_string()),
        ("arrow", arrow_name(config.arrow).to_string()),
    ] {
        out.push_str(&format!("% xitoc {key}={value}\n"));
    }
    // The native renderer implements these PGFPlots-style axis tick semantics
    // directly. Comments keep the editable TikZ subset TeX-independent.
    if config.show_zero {
        out.push_str(&format!(
            "% xitoc xtick={{0}}\n% xitoc xticklabels={{${}$}}\n% xitoc yticklabels={{}}\n",
            config.origin_label
        ));
    } else {
        out.push_str("% xitoc xtick={}\n% xitoc xticklabels={}\n% xitoc yticklabels={}\n");
    }
    out.push_str(&format!(
        "% xitoc surface-count={}\n% xitoc marker-count={}\n",
        config.surfaces.len(),
        config.markers.len()
    ));
    for (i, axis) in config.axes.iter().enumerate() {
        let name = ["x", "y", "z"][i];
        out.push_str(&format!("% xitoc axis-{name}-range={}:{}\n% xitoc axis-{name}-visible={}\n% xitoc axis-{name}-label={}\n% xitoc axis-{name}-show-label={}\n",
            axis.range[0], axis.range[1], axis.visible, axis.label.replace(' ', "_"), axis.show_label));
    }
    for (key, value) in [
        (
            "axis-color",
            config.axis_color.map_or_else(|| "default".into(), rgb),
        ),
        ("grid-color", rgb(config.grid_color)),
        (
            "tick-color",
            config.tick_color.map_or_else(|| "default".into(), rgb),
        ),
        (
            "background",
            config.background.map_or_else(|| "transparent".into(), rgb),
        ),
    ] {
        out.push_str(&format!("% xitoc {key}={value}\n"));
    }

    // These draw commands are intentionally ordinary, editable TikZ syntax.
    for (index, axis) in config
        .axes
        .iter()
        .enumerate()
        .take(config.preset.dimension())
    {
        if !axis.visible {
            continue;
        }
        let color = config.axis_color.map_or_else(|| "black".into(), rgb);
        let option = format!(
            "{color}, line width={}pt, {}",
            config.axis_width,
            match config.arrow {
                Arrow::Scientific => "->",
                Arrow::Open => "open",
                Arrow::None => "-",
            }
        );
        match index {
            0 if config.preset.dimension() == 1 => {
                if axis.show_label {
                    out.push_str(&format!(
                        "\\draw[{option}] ({},0) -- ({},0) node[right] {{$\\mathit{{{}}}$}};\n",
                        axis.range[0], axis.range[1], axis.label
                    ));
                } else {
                    out.push_str(&format!(
                        "\\draw[{option}] ({},0) -- ({},0);\n",
                        axis.range[0], axis.range[1]
                    ));
                }
            }
            0 if axis.show_label => out.push_str(&format!(
                "\\draw[{option}] ({},0) -- ({},0) node[right] {{$\\mathit{{{}}}$}};\n",
                axis.range[0], axis.range[1], axis.label
            )),
            0 => out.push_str(&format!(
                "\\draw[{option}] ({},0) -- ({},0);\n",
                axis.range[0], axis.range[1]
            )),
            1 if config.preset.dimension() == 3 => out.push_str(&format!(
                "% xitoc projected-y-axis range={}..{} label={}\n",
                axis.range[0],
                axis.range[1],
                axis.label.replace(' ', "_")
            )),
            1 if axis.show_label => out.push_str(&format!(
                "\\draw[{option}] (0,{}) -- (0,{}) node[above] {{$\\mathit{{{}}}$}};\n",
                axis.range[0], axis.range[1], axis.label
            )),
            1 => out.push_str(&format!(
                "\\draw[{option}] (0,{}) -- (0,{});\n",
                axis.range[0], axis.range[1]
            )),
            _ => out.push_str(&format!(
                "% xitoc z-axis range={}..{} label={}\n",
                axis.range[0],
                axis.range[1],
                axis.label.replace(' ', "_")
            )),
        }
    }
    if config.grid && config.preset.dimension() >= 2 {
        out.push_str(&format!(
            "\\draw[{}, step={}, opacity={}] ({},{}) grid ({},{});\n",
            rgb(config.grid_color),
            config.grid_step,
            f64::from(config.grid_opacity) / 255.,
            config.axes[0].range[0],
            config.axes[1].range[0],
            config.axes[0].range[1],
            config.axes[1].range[1]
        ));
    }
    for (i, plot) in config.curves.iter().enumerate() {
        if plot.visible {
            let domain = plot.domain.unwrap_or(config.axes[0].range);
            out.push_str(&format!("\\draw[{}, line width={}pt, {}, domain={}:{}] plot (\\x, {{{}}}); % xitoc curve-{i}\n",
                rgb(plot.color), plot.width, pattern_name(plot.pattern), domain[0], domain[1], tikz_expression(&plot.expression)));
        }
    }
    for (i, plot) in config.surfaces.iter().enumerate() {
        let xd = plot.domain.unwrap_or(config.axes[0].range);
        let yd = plot.y_domain.unwrap_or(config.axes[1].range);
        out.push_str(&format!(
            "% xitoc surface-{i}={} ; {} ; {}:{} ; {}:{} ; {} ; {} ; {}\n",
            plot.visible,
            plot.expression.replace(' ', ""),
            xd[0],
            xd[1],
            yd[0],
            yd[1],
            rgb(plot.color),
            plot.width,
            pattern_name(plot.pattern)
        ));
    }
    for (i, marker) in config.markers.iter().enumerate() {
        out.push_str(&format!(
            "% xitoc marker-{i}={} ; {} ; {} ; {} ; {}\n",
            marker.start,
            marker.end.map_or_else(|| "none".into(), |n| n.to_string()),
            marker.closed[0],
            marker.closed[1],
            rgb(marker.color)
        ));
    }
    out.push_str("\\end{tikzpicture}\n");
    out
}

/// Apply a supported source edit to an existing configuration. This parser
/// intentionally rejects unknown commands and incomplete input; callers keep
/// the last valid preview and the editor buffer on error.
pub fn parse(source: &str, base: &Config) -> Result<Config, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("TikZ source is limited to 32 KiB.".into());
    }
    if !source.contains("\\begin{tikzpicture}") || !source.contains("\\end{tikzpicture}") {
        return Err("Add a complete \\begin{tikzpicture} … \\end{tikzpicture} environment.".into());
    }
    if source.matches("\\begin{").count() != 1 || source.matches("\\end{").count() != 1 {
        return Err("Only one tikzpicture environment is supported.".into());
    }
    let mut config = base.clone();
    let mut axes = [None, None];
    let mut curves: Vec<Plot> = Vec::new();
    let mut grid_seen = false;
    let mut origin_seen = false;
    let mut xtick_zero = None;
    let mut xticklabels_visible = None;
    let mut explicit_show_zero = None;
    let mut commands = Vec::new();
    let mut pending = String::new();
    let mut pending_line = 0;
    for (line_no, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("% xitoc ") {
            apply_directive(line, &mut config, line_no)?;
            if let Some((key, value)) = line.trim_start_matches("% xitoc ").split_once('=') {
                match key.trim() {
                    "xtick" => {
                        xtick_zero = Some(
                            value
                                .replace(['{', '}'], "")
                                .split(',')
                                .any(|v| v.trim() == "0"),
                        );
                    }
                    "xticklabels" => {
                        let label = value
                            .trim()
                            .trim_start_matches('{')
                            .trim_end_matches('}')
                            .trim();
                        xticklabels_visible = Some(!label.is_empty());
                    }
                    "show-zero" | "origin" => {
                        explicit_show_zero = Some(parse_bool(value.trim(), line_no)?);
                    }
                    _ => {}
                }
            }
            continue;
        }
        if line.is_empty()
            || line.starts_with('%')
            || line.starts_with("\\begin{")
            || line.starts_with("\\end{")
        {
            continue;
        }
        if pending.is_empty() && !line.starts_with("\\draw") && !line.starts_with("\\node") {
            return Err(at(
                line_no,
                "unsupported TikZ command; only draw and node commands are supported",
            ));
        }
        if pending.is_empty() {
            pending_line = line_no;
        }
        pending.push(' ');
        pending.push_str(line.split('%').next().unwrap_or(line).trim());
        if pending.contains(';') {
            commands.push((pending_line, std::mem::take(&mut pending)));
        }
        if commands.len() > 512 {
            return Err("TikZ source contains more than 512 commands.".into());
        }
    }
    if !pending.trim().is_empty() {
        return Err(at(
            pending_line,
            "incomplete command; finish it with a semicolon",
        ));
    }
    for (line_no, command) in commands {
        let line = command.trim();
        if line.starts_with("\\draw") {
            if line.contains(" grid ") {
                grid_seen = true;
                config.grid = true;
                let opts = options(line);
                if let Some(step) = opts.get("step") {
                    config.grid_step = parse_num(step, line_no)?;
                }
                if let Some(c) = opts.get("color") {
                    config.grid_color = parse_color(c, line_no)?;
                }
                if let Some(opacity) = opts.get("opacity") {
                    config.grid_opacity = (parse_num(opacity, line_no)?.clamp(0., 1.) * 255.) as u8;
                }
                if let Some(width) = opts.get("line width") {
                    config.grid_width = parse_num(width.trim_end_matches("pt"), line_no)?;
                }
                continue;
            }
            if line.contains("plot") {
                let options = options(line);
                let color = options
                    .get("color")
                    .or_else(|| options.get("0"))
                    .map_or(Ok([46, 112, 210]), |v| parse_color(v, line_no))?;
                let width = options
                    .get("line width")
                    .map_or(Ok(1.5), |v| parse_num(v.trim_end_matches("pt"), line_no))?;
                let width = if options.contains_key("very thick") {
                    2.5
                } else if options.contains_key("thick") {
                    1.5
                } else if options.contains_key("thin") {
                    0.5
                } else {
                    width
                };
                let pattern = if options.contains_key("dashed") {
                    Pattern::Dashed
                } else if options.contains_key("dotted") {
                    Pattern::Dotted
                } else {
                    Pattern::Solid
                };
                let domain = options
                    .get("domain")
                    .map(|v| parse_range(v, line_no))
                    .transpose()?;
                let expr = between(line, "plot (\\x, {", "})")
                    .ok_or_else(|| at(line_no, "expected plot (\\x, {expression})"))?;
                let mut plot = Plot::new(curves.len(), false);
                plot.expression = expr.trim().replace("\\x", "x");
                plot.color = color;
                plot.width = width;
                plot.pattern = pattern;
                plot.domain = domain.filter(|range| {
                    *range
                        != base
                            .curves
                            .get(curves.len())
                            .and_then(|p| p.domain)
                            .unwrap_or(base.axes[0].range)
                });
                curves.push(plot);
                continue;
            }
            let coords = coordinate_pairs(line, line_no)?;
            if coords.len() < 2 {
                return Err(at(line_no, "expected two axis endpoints"));
            }
            let ((x0, y0), (x1, y1)) = (coords[0], coords[1]);
            let axis = if y0.abs() < 1e-9 && y1.abs() < 1e-9 {
                0
            } else if x0.abs() < 1e-9 && x1.abs() < 1e-9 {
                1
            } else {
                return Err(at(
                    line_no,
                    "only horizontal or vertical coordinate axes are supported",
                ));
            };
            let (lo, hi) = if axis == 0 {
                (x0.min(x1), x0.max(x1))
            } else {
                (y0.min(y1), y0.max(y1))
            };
            config.axes[axis].range = [lo, hi];
            axes[axis] = Some(());
            let opts = options(line);
            if let Some(c) = opts.get("color").or_else(|| opts.get("0")) {
                let parsed = parse_color(c, line_no)?;
                config.axis_color = if c == "black" && base.axis_color.is_none() {
                    None
                } else {
                    Some(parsed)
                };
            }
            if let Some(w) = opts.get("line width") {
                config.axis_width = parse_num(w.trim_end_matches("pt"), line_no)?;
            } else if opts.contains_key("very thick") {
                config.axis_width = 2.5;
            } else if opts.contains_key("thick") {
                config.axis_width = 1.5;
            } else if opts.contains_key("thin") {
                config.axis_width = 0.5;
            }
            let label = axis_label(line);
            config.axes[axis].show_label = label.is_some();
            if let Some(label) = label {
                config.axes[axis].label = label;
            }
            if opts.contains_key("->") {
                config.arrow = Arrow::Scientific;
            } else if opts.contains_key("- |") || opts.contains_key("open") {
                config.arrow = Arrow::Open;
            } else if opts.contains_key("-") {
                config.arrow = Arrow::None;
            }
            continue;
        }
        if line.starts_with("\\node") {
            origin_seen = true;
            if coordinate_pairs(line, line_no)? != [(0., 0.)] {
                return Err(at(line_no, "only an origin node at (0,0) is supported"));
            }
            if let Some(label) = between(line, "$", "$") {
                config.origin_label = label.trim().to_owned();
            }
            config.show_zero = true;
            continue;
        }
        if line.starts_with("% xitoc ") {
            apply_directive(line, &mut config, line_no)?;
            continue;
        }
        return Err(at(
            line_no,
            "unsupported TikZ command; supported commands are axes, grids, nodes and function plots",
        ));
    }
    if config.preset.dimension() == 2 {
        curves.extend(base.curves.iter().filter(|plot| !plot.visible).cloned());
        config.curves = curves;
    }
    if config.preset.dimension() < 3 {
        for (i, axis) in axes.iter().enumerate().take(config.preset.dimension()) {
            config.axes[i].visible = axis.is_some();
        }
    }
    if config.preset.dimension() >= 2 {
        config.grid = grid_seen;
    }
    // Old documents used an independently positioned origin node (and an
    // `origin` directive). Honor that source on import, then normalize the
    // state to tick-based zero labeling going forward.
    config.show_zero = match (xtick_zero, xticklabels_visible) {
        (Some(tick), Some(label)) => tick && label,
        (Some(tick), None) => tick,
        (None, Some(label)) => label,
        (None, None) => explicit_show_zero.unwrap_or(origin_seen || config.show_zero),
    };
    config.origin = config.show_zero;
    config
        .validate()
        .map_err(|e| format!("TikZ configuration: {e}"))?;
    Ok(config)
}

fn apply_directive(line: &str, c: &mut Config, n: usize) -> Result<(), String> {
    let Some((key, value)) = line.trim_start_matches("% xitoc ").split_once('=') else {
        return Ok(());
    };
    let value = value.trim();
    let number = || parse_num(value, n);
    match key.trim() {
        "preset" => {
            let p = match value {
                "Positive" => Preset::Positive,
                "Signed" => Preset::Signed,
                "FirstQuadrant" => Preset::FirstQuadrant,
                "FourQuadrants" => Preset::FourQuadrants,
                "Octant" => Preset::Octant,
                _ => return Err(at(n, "unknown preset")),
            };
            c.set_preset(p);
        }
        "azimuth" => c.azimuth = number()?,
        "elevation" => c.elevation = number()?,
        "equal-units" => c.equal_units = parse_bool(value, n)?,
        "numbers" => c.numbers = parse_bool(value, n)?,
        "show-zero" | "origin" => {
            c.show_zero = parse_bool(value, n)?;
            c.origin = c.show_zero;
        }
        "xtick" => {
            c.show_zero = value
                .replace(['{', '}'], "")
                .split(',')
                .any(|v| v.trim() == "0");
            c.origin = c.show_zero;
        }
        "xticklabels" => {
            let label = value
                .trim()
                .trim_start_matches('{')
                .trim_end_matches('}')
                .trim();
            c.show_zero = !label.is_empty();
            c.origin = c.show_zero;
            if let Some(label) = label.strip_prefix('$').and_then(|s| s.strip_suffix('$')) {
                c.origin_label = label.to_owned();
            }
        }
        "yticklabels" => { /* zero is intentionally labeled on X only */ }
        "origin-label" => c.origin_label = value.replace('_', " "),
        "ticks" => c.ticks = parse_bool(value, n)?,
        "tick-step" => c.tick_step = number()?,
        "tick-length" => c.tick_length = number()?,
        "tick-width" => c.tick_width = number()?,
        "grid" => c.grid = parse_bool(value, n)?,
        "grid-step" => c.grid_step = number()?,
        "grid-width" => c.grid_width = number()?,
        "grid-opacity" => c.grid_opacity = (number()?.clamp(0., 255.)) as u8,
        "grid-subdivisions" => c.grid_subdivisions = (number()?.clamp(1., 8.)) as u8,
        "axis-width" => c.axis_width = number()?,
        "arrow" => {
            c.arrow = match value {
                "Scientific" => Arrow::Scientific,
                "Open" => Arrow::Open,
                "None" => Arrow::None,
                _ => return Err(at(n, "unknown arrow style")),
            }
        }
        "axis-color" => {
            c.axis_color = if value == "default" {
                None
            } else {
                Some(parse_color(value, n)?)
            }
        }
        "grid-color" => c.grid_color = parse_color(value, n)?,
        "tick-color" => {
            c.tick_color = if value == "default" {
                None
            } else {
                Some(parse_color(value, n)?)
            }
        }
        "background" => {
            c.background = if value == "transparent" {
                None
            } else {
                Some(parse_color(value, n)?)
            }
        }
        "surface-count" => {
            let count = number()?.clamp(0., crate::coordinate::MAX_PLOTS as f64) as usize;
            c.surfaces.truncate(count);
            while c.surfaces.len() < count {
                c.surfaces.push(Plot::new(c.surfaces.len(), true));
            }
        }
        "marker-count" => {
            let count = number()?.clamp(0., crate::coordinate::MAX_MARKERS as f64) as usize;
            c.markers.truncate(count);
            while c.markers.len() < count {
                c.markers.push(crate::coordinate::Marker {
                    start: 1.,
                    end: None,
                    closed: [true, true],
                    color: [46, 112, 210],
                });
            }
        }
        "axis-x-range" => c.axes[0].range = parse_range(value, n)?,
        "axis-y-range" => c.axes[1].range = parse_range(value, n)?,
        "axis-z-range" => c.axes[2].range = parse_range(value, n)?,
        "axis-x-visible" => c.axes[0].visible = parse_bool(value, n)?,
        "axis-y-visible" => c.axes[1].visible = parse_bool(value, n)?,
        "axis-z-visible" => c.axes[2].visible = parse_bool(value, n)?,
        "axis-x-label" => c.axes[0].label = value.replace('_', " "),
        "axis-y-label" => c.axes[1].label = value.replace('_', " "),
        "axis-z-label" => c.axes[2].label = value.replace('_', " "),
        "axis-x-show-label" => c.axes[0].show_label = parse_bool(value, n)?,
        "axis-y-show-label" => c.axes[1].show_label = parse_bool(value, n)?,
        "axis-z-show-label" => c.axes[2].show_label = parse_bool(value, n)?,
        key if key.starts_with("surface-") => {
            let index = key
                .trim_start_matches("surface-")
                .parse::<usize>()
                .map_err(|_| at(n, "invalid surface index"))?;
            if index >= crate::coordinate::MAX_PLOTS {
                return Err(at(n, "surface index exceeds limit"));
            }
            let parts = value.split(';').map(str::trim).collect::<Vec<_>>();
            if parts.len() != 7 {
                return Err(at(
                    n,
                    "surface entry needs visibility, expression, X range, Y range, color, width and line style",
                ));
            }
            let previous = c
                .surfaces
                .get(index)
                .cloned()
                .unwrap_or_else(|| Plot::new(index, true));
            while c.surfaces.len() <= index {
                c.surfaces.push(Plot::new(c.surfaces.len(), true));
            }
            let plot = &mut c.surfaces[index];
            plot.visible = parse_bool(parts[0], n)?;
            plot.expression = parts[1].to_owned();
            let x_domain = parse_range(parts[2], n)?;
            let y_domain = parse_range(parts[3], n)?;
            plot.domain = if x_domain == previous.domain.unwrap_or(c.axes[0].range) {
                None
            } else {
                Some(x_domain)
            };
            plot.y_domain = if y_domain == previous.y_domain.unwrap_or(c.axes[1].range) {
                None
            } else {
                Some(y_domain)
            };
            plot.color = parse_color(parts[4], n)?;
            plot.width = parse_num(parts[5], n)?;
            plot.pattern = parse_pattern(parts[6], n)?;
        }
        key if key.starts_with("marker-") => {
            let index = key
                .trim_start_matches("marker-")
                .parse::<usize>()
                .map_err(|_| at(n, "invalid marker index"))?;
            if index >= crate::coordinate::MAX_MARKERS {
                return Err(at(n, "marker index exceeds limit"));
            }
            let parts = value.split(';').map(str::trim).collect::<Vec<_>>();
            if parts.len() != 5 {
                return Err(at(
                    n,
                    "marker entry needs start, end, endpoint states and color",
                ));
            }
            while c.markers.len() <= index {
                c.markers.push(crate::coordinate::Marker {
                    start: 1.,
                    end: None,
                    closed: [true, true],
                    color: [46, 112, 210],
                });
            }
            let marker = &mut c.markers[index];
            marker.start = parse_num(parts[0], n)?;
            marker.end = if parts[1] == "none" {
                None
            } else {
                Some(parse_num(parts[1], n)?)
            };
            marker.closed = [parse_bool(parts[2], n)?, parse_bool(parts[3], n)?];
            marker.color = parse_color(parts[4], n)?;
        }
        key if key.starts_with("projected-y-axis") || key.starts_with("z-axis") => {}
        _ => return Err(at(n, "unsupported XiToc setting")),
    }
    Ok(())
}

fn options(line: &str) -> std::collections::HashMap<String, String> {
    let Some(raw) = between(line, "[", "]") else {
        return Default::default();
    };
    let mut map = std::collections::HashMap::new();
    for item in raw.split(',') {
        let item = item.trim();
        if let Some((k, v)) = item.split_once('=') {
            map.insert(k.trim().to_owned(), v.trim().to_owned());
        } else if matches!(
            item,
            "->" | "-" | "open" | "dashed" | "dotted" | "solid" | "thin" | "thick" | "very thick"
        ) {
            map.insert(item.to_owned(), String::new());
        } else if item.starts_with('#')
            || matches!(
                item,
                "black"
                    | "white"
                    | "red"
                    | "blue"
                    | "green"
                    | "gray"
                    | "grey"
                    | "orange"
                    | "purple"
            )
        {
            map.insert("color".into(), item.into());
        } else {
            map.insert(item.to_owned(), String::new());
        }
    }
    map
}
fn coordinate_pairs(line: &str, n: usize) -> Result<Vec<(f64, f64)>, String> {
    let mut out = Vec::new();
    let mut rem = line;
    while let Some((_, r)) = rem.split_once('(') {
        let Some((pair, tail)) = r.split_once(')') else {
            return Err(at(n, "incomplete coordinate"));
        };
        let Some((x, y)) = pair.split_once(',') else {
            return Err(at(n, "coordinate must be (x,y)"));
        };
        out.push((parse_num(x.trim(), n)?, parse_num(y.trim(), n)?));
        rem = tail;
        if out.len() > 16 {
            return Err(at(n, "too many coordinates"));
        }
    }
    Ok(out)
}
fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    Some(s.split_once(a)?.1.split_once(b)?.0)
}
fn axis_label(line: &str) -> Option<String> {
    let node = line.split_once("node[")?.1;
    let source = node.split_once('$')?.1.split_once('$')?.0;
    let label = source
        .strip_prefix("\\mathit{")
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(source);
    (!label.is_empty()).then(|| label.to_owned())
}
fn parse_range(s: &str, n: usize) -> Result<[f64; 2], String> {
    let (a, b) = s
        .split_once(':')
        .or_else(|| s.split_once(".."))
        .ok_or_else(|| at(n, "expected start:end range"))?;
    Ok([parse_num(a, n)?, parse_num(b, n)?])
}
fn parse_num(s: &str, n: usize) -> Result<f64, String> {
    let v = s
        .trim()
        .trim_end_matches("pt")
        .trim_end_matches("cm")
        .parse::<f64>()
        .map_err(|_| at(n, "invalid number"))?;
    if !v.is_finite() || v.abs() > 1e6 {
        Err(at(n, "number is outside the supported range"))
    } else {
        Ok(v)
    }
}
fn parse_bool(s: &str, n: usize) -> Result<bool, String> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(at(n, "expected true or false")),
    }
}
fn parse_color(s: &str, n: usize) -> Result<[u8; 3], String> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#')
        && h.len() == 6
        && h.is_ascii()
    {
        let mut c = [0; 3];
        for i in 0..3 {
            c[i] =
                u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).map_err(|_| at(n, "invalid color"))?;
        }
        return Ok(c);
    }
    match s {
        "black" => Ok([0, 0, 0]),
        "white" => Ok([255, 255, 255]),
        "red" => Ok([220, 40, 50]),
        "blue" => Ok([40, 100, 220]),
        "green" => Ok([30, 140, 80]),
        "gray" | "grey" => Ok([128, 128, 128]),
        "orange" => Ok([230, 130, 20]),
        "purple" => Ok([135, 70, 190]),
        _ => Err(at(
            n,
            "unsupported color; use black, white, red, blue, green, gray, orange, purple or #RRGGBB",
        )),
    }
}
fn rgb(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
fn preset_name(p: Preset) -> &'static str {
    match p {
        Preset::Positive => "Positive",
        Preset::Signed => "Signed",
        Preset::FirstQuadrant => "FirstQuadrant",
        Preset::FourQuadrants => "FourQuadrants",
        Preset::Octant => "Octant",
    }
}
fn arrow_name(a: Arrow) -> &'static str {
    match a {
        Arrow::Scientific => "Scientific",
        Arrow::Open => "Open",
        Arrow::None => "None",
    }
}
fn pattern_name(p: Pattern) -> &'static str {
    match p {
        Pattern::Solid => "solid",
        Pattern::Dashed => "dashed",
        Pattern::Dotted => "dotted",
    }
}
fn parse_pattern(value: &str, n: usize) -> Result<Pattern, String> {
    match value {
        "solid" => Ok(Pattern::Solid),
        "dashed" => Ok(Pattern::Dashed),
        "dotted" => Ok(Pattern::Dotted),
        _ => Err(at(n, "unsupported line style")),
    }
}
fn expression(s: &str) -> String {
    s.trim()
        .strip_prefix("y=")
        .or_else(|| s.trim().strip_prefix("y ="))
        .or_else(|| s.trim().strip_prefix("z="))
        .or_else(|| s.trim().strip_prefix("z ="))
        .unwrap_or(s)
        .trim()
        .to_owned()
}
fn tikz_expression(source: &str) -> String {
    let source = expression(source);
    let chars = source.char_indices().collect::<Vec<_>>();
    let mut out = String::with_capacity(source.len() + 8);
    for (i, (pos, ch)) in chars.iter().enumerate() {
        if *ch == 'x' {
            let prev = chars.get(i.wrapping_sub(1)).map(|(_, c)| *c);
            let next = chars.get(i + 1).map(|(_, c)| *c);
            if !prev.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                && !next.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                out.push('\\');
            }
        }
        let end = chars.get(i + 1).map_or(source.len(), |(p, _)| *p);
        out.push_str(&source[*pos..end]);
    }
    out
}
fn at(n: usize, msg: &str) -> String {
    format!("TikZ line {}: {msg}.", n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_source_roundtrips_all_presets() {
        for p in Preset::ALL {
            let c = Config::new(p);
            let s = generate(&c);
            assert_eq!(parse(&s, &c).unwrap(), c);
            assert!(!s.contains("\\node"));
            assert!(s.contains("% xitoc xtick={0}"));
            assert!(s.contains("% xitoc xticklabels={$0$}"));
        }
    }
    #[test]
    fn independent_tick_label_flags_roundtrip_and_source_edits_sync() {
        let mut base = Config::new(Preset::FourQuadrants);
        for (numbers, show_zero) in [(false, true), (false, false), (true, true), (true, false)] {
            base.numbers = numbers;
            base.show_zero = show_zero;
            base.origin = show_zero;
            let source = generate(&base);
            assert_eq!(parse(&source, &base).unwrap(), base);
        }
        base.numbers = true;
        base.show_zero = false;
        let source = generate(&base)
            .replace("% xitoc numbers=true", "% xitoc numbers=false")
            .replace("% xitoc xtick={}", "% xitoc xtick={0}")
            .replace("% xitoc xticklabels={}", "% xitoc xticklabels={$0$}");
        let parsed = parse(&source, &base).unwrap();
        assert!(!parsed.numbers);
        assert!(parsed.show_zero);
        assert_eq!(parsed.origin_label, "0");
    }
    #[test]
    fn edited_axis_and_plot_commands_change_model() {
        let mut c = Config::new(Preset::FourQuadrants);
        c.curves.push(Plot::new(0, false));
        let s = generate(&c)
            .replace("(-5,0) -- (5,0)", "(-10,0) -- (10,0)")
            .replace(
                "#2e70d2, line width=1.5pt, solid, domain=-5:5",
                "red, line width=2pt, dashed, domain=-3:3",
            )
            .replace("{\\x^2}", "{\\x^3}");
        let p = parse(&s, &c).unwrap();
        assert_eq!(p.axes[0].range, [-10., 10.]);
        assert_eq!(p.curves[0].expression, "x^3");
        assert_eq!(p.curves[0].color, [220, 40, 50]);
        assert_eq!(p.curves[0].width, 2.);
        assert_eq!(p.curves[0].pattern, Pattern::Dashed);
        assert_eq!(p.curves[0].domain, Some([-3., 3.]));
    }
    #[test]
    fn generated_native_dimension_data_roundtrips_and_source_comments_survive_regeneration() {
        let mut c = Config::new(Preset::Octant);
        c.surfaces.push(Plot::new(0, true));
        c.surfaces[0].expression = "sin(x)*cos(y)".into();
        c.surfaces[0].pattern = Pattern::Dotted;
        c.markers.push(crate::coordinate::Marker {
            start: -2.,
            end: Some(3.),
            closed: [false, true],
            color: [30, 80, 180],
        });
        let source = generate(&c);
        assert_eq!(
            parse(&source, &c).unwrap_or_else(|e| panic!("{e}\n{source}")),
            c
        );
        let edited = source.replace("surface-0=true ; sin(x)*cos(y)", "surface-0=true ; x*y");
        assert_eq!(parse(&edited, &c).unwrap().surfaces[0].expression, "x*y");
        let kept = generate_preserving(
            &c,
            "\\begin{tikzpicture}\n% keep this note\n\\end{tikzpicture}\n",
        );
        assert!(kept.contains("% keep this note"));
    }
    #[test]
    fn standard_multiline_plot_and_axis_labels_are_accepted() {
        let base = Config::new(Preset::FourQuadrants);
        let text = r#"\begin{tikzpicture}
\draw[red, very thick, ->] (-8,0) -- (8,0) node[right] {$t$};
\draw[blue, very thick, domain=-2:2, dashed]
  plot (\x, {\x^3});
\end{tikzpicture}"#;
        let parsed = parse(text, &base).unwrap();
        assert_eq!(parsed.axes[0].range, [-8., 8.]);
        assert_eq!(parsed.axes[0].label, "t");
        assert_eq!(parsed.axis_width, 2.5);
        assert_eq!(parsed.curves[0].expression, "x^3");
        assert_eq!(parsed.curves[0].width, 2.5);
    }
    #[test]
    fn malformed_and_unsupported_source_is_preserved_by_error() {
        let c = Config::new(Preset::FourQuadrants);
        assert!(parse("\\begin{tikzpicture}\n\\draw[blue\n", &c).is_err());
        assert!(
            parse(
                "\\begin{tikzpicture}\n\\input{secret}\n\\end{tikzpicture}",
                &c
            )
            .unwrap_err()
            .contains("unsupported TikZ")
        );
    }
}
