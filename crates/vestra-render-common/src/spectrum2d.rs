//! Shared rasterization of path-based Spectrum2D sources and waves.

use image::RgbaImage;
use tiny_skia::{
    Color, FillRule, GradientStop, IntSize, LineCap, LineJoin, LinearGradient, Paint, Path,
    PathBuilder, Pixmap, SpreadMode, Stroke, Transform,
};
use vestra_core::{
    Category, Diagnostic,
    domain::Point,
    plan::{EvaluatedSource, EvaluatedSpectrum2DWave},
    project::{
        Spectrum2DBandMapping, Spectrum2DGradientDirection, Spectrum2DLayout,
        Spectrum2DLinearAnchor, Spectrum2DPathBandMapping, Spectrum2DPathDirection,
        Spectrum2DRadialDirection, Spectrum2DStyle, parse_colour,
    },
};

type Gradient = (Spectrum2DGradientDirection, [u8; 4], [u8; 4]);

/// Legacy linear and radial bars retain their dedicated renderer paths.
pub fn uses_path_raster(layout: &Spectrum2DLayout, has_wave: bool) -> bool {
    has_wave
        || matches!(
            layout,
            Spectrum2DLayout::Ellipse(_) | Spectrum2DLayout::Path(_)
        )
}

/// Draws the evaluated source in its containing composition's local surface.
pub fn rasterize(
    source: &EvaluatedSource,
    width: u32,
    height: u32,
) -> Result<RgbaImage, Diagnostic> {
    let EvaluatedSource::Spectrum2D {
        bands,
        x,
        y,
        width: region_width,
        height: region_height,
        bar_gap_ratio,
        min_bar_height_ratio,
        layout,
        gradient,
        colour,
        wave,
    } = source
    else {
        return Err(Diagnostic::error(
            "VESTRA-SPECTRUM2D-SOURCE",
            Category::Internal,
            "spectrum rasterization requires an evaluated Spectrum2D source",
            "",
        ));
    };
    let mut pixels = empty_surface(width, height)?;
    let path = Baseline::new(
        layout,
        [
            x * f64::from(width),
            y * f64::from(height),
            region_width * f64::from(width),
            region_height * f64::from(height),
        ],
    );
    let style = wave
        .as_ref()
        .map_or(Spectrum2DStyle::Bars, |wave| wave.style);
    if style != Spectrum2DStyle::Wave {
        let opacity = wave.as_ref().map_or(1.0, |wave| wave.settings.bar_opacity);
        draw_bars(
            &mut pixels,
            &path,
            bands,
            *bar_gap_ratio,
            *min_bar_height_ratio,
            *colour,
            *gradient,
            opacity,
        );
    }
    if let Some(wave) = wave.as_ref().filter(|_| style != Spectrum2DStyle::Bars) {
        draw_waves(&mut pixels, &path, bands, *colour, wave);
    }
    let mut rgba = pixels.take();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if (1..255).contains(&alpha) {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Ok(RgbaImage::from_raw(width, height, rgba)
        .expect("validated pixmap has exact RGBA dimensions"))
}

fn empty_surface(width: u32, height: u32) -> Result<Pixmap, Diagnostic> {
    let unsupported = || {
        Diagnostic::error(
            "VESTRA-SPECTRUM2D-SURFACE",
            Category::Render,
            format!("cannot allocate a {width}×{height} spectrum surface"),
            "",
        )
    };
    let size = IntSize::from_wh(width, height).ok_or_else(unsupported)?;
    let stride = width
        .checked_mul(4)
        .filter(|stride| i32::try_from(*stride).is_ok())
        .ok_or_else(unsupported)?;
    let length =
        usize::try_from(u64::from(stride) * u64::from(height)).map_err(|_| unsupported())?;
    let mut data = Vec::new();
    data.try_reserve_exact(length).map_err(|_| unsupported())?;
    data.resize(length, 0);
    Pixmap::from_vec(data, size).ok_or_else(unsupported)
}

fn path_order(mapping: Spectrum2DPathBandMapping) -> Spectrum2DBandMapping {
    match mapping {
        Spectrum2DPathBandMapping::Forward => Spectrum2DBandMapping::Forward,
        Spectrum2DPathBandMapping::Reverse => Spectrum2DBandMapping::Reverse,
    }
}

struct Baseline {
    origin: Point,
    vertices: Vec<Point>,
    distances: Vec<f64>,
    normals: Vec<Point>,
    closed: bool,
    reverse_normal: bool,
    two_sided: bool,
    reach: f64,
    order: Spectrum2DBandMapping,
}

impl Baseline {
    fn new(layout: &Spectrum2DLayout, [x, y, width, height]: [f64; 4]) -> Self {
        let mut baseline = Self {
            origin: point(x, y),
            vertices: Vec::new(),
            distances: Vec::new(),
            normals: Vec::new(),
            closed: false,
            reverse_normal: false,
            two_sided: false,
            reach: height,
            order: Spectrum2DBandMapping::Forward,
        };
        match layout {
            Spectrum2DLayout::Linear(layout) => {
                let y = match layout.anchor {
                    Spectrum2DLinearAnchor::Bottom => height,
                    Spectrum2DLinearAnchor::Top => 0.0,
                    Spectrum2DLinearAnchor::Center => height / 2.0,
                };
                baseline.vertices = vec![point(0.0, y), point(width, y)];
                baseline.two_sided = layout.anchor == Spectrum2DLinearAnchor::Center;
                baseline.reverse_normal = layout.anchor == Spectrum2DLinearAnchor::Top;
                baseline.order = layout.band_mapping;
            }
            Spectrum2DLayout::Radial(layout) => {
                baseline.origin = point(x + width / 2.0, y + height / 2.0);
                let outer = width.min(height) / 2.0;
                let inner = outer * layout.inner_radius_ratio;
                let radius = match layout.direction {
                    Spectrum2DRadialDirection::Outward => inner,
                    Spectrum2DRadialDirection::Inward => outer,
                    Spectrum2DRadialDirection::Both => (inner + outer) / 2.0,
                };
                baseline.reach = outer - inner;
                baseline.order = layout.band_mapping;
                baseline.reverse_normal = layout.direction == Spectrum2DRadialDirection::Inward;
                baseline.two_sided = layout.direction == Spectrum2DRadialDirection::Both;
                baseline.add_ellipse(
                    radius,
                    radius,
                    layout.start_angle_degrees,
                    layout.sweep_angle_degrees,
                );
            }
            Spectrum2DLayout::Ellipse(layout) => {
                baseline.origin = point(x + width / 2.0, y + height / 2.0);
                baseline.order = path_order(layout.band_mapping);
                baseline.reverse_normal = layout.direction == Spectrum2DRadialDirection::Inward;
                baseline.two_sided = layout.direction == Spectrum2DRadialDirection::Both;
                baseline.add_ellipse(
                    width * layout.radius_x,
                    height * layout.radius_y,
                    layout.start_angle_degrees,
                    layout.sweep_angle_degrees,
                );
            }
            Spectrum2DLayout::Path(layout) => {
                baseline.order = path_order(layout.band_mapping);
                baseline.vertices = layout
                    .points
                    .iter()
                    .map(|p| point(width * p.x, height * p.y))
                    .collect();
                baseline.closed = layout.closed;
                baseline.reverse_normal = layout.direction == Spectrum2DPathDirection::Right;
                baseline.two_sided = layout.direction == Spectrum2DPathDirection::Both;
                if baseline.closed
                    && let Some(&first) = baseline.vertices.first()
                {
                    baseline.vertices.push(first);
                }
            }
        }
        if baseline.distances.is_empty() {
            baseline.distances.push(0.0);
            for pair in baseline.vertices.windows(2) {
                let delta = subtract(pair[1], pair[0]);
                baseline.distances.push(
                    baseline.distances[baseline.distances.len() - 1] + delta.x.hypot(delta.y),
                );
            }
        }
        if baseline.two_sided {
            baseline.reach *= 0.5;
        }
        baseline
    }

    fn add_ellipse(&mut self, rx: f64, ry: f64, start: f64, sweep: f64) {
        self.closed = sweep == 360.0;
        // Even parameter steps give a bounded chord table. Distance lookup then evens out band spacing.
        let count = (512.0 * sweep / 360.0).ceil().max(1.0) as usize;
        let start = start.rem_euclid(360.0).to_radians();
        let step = sweep.to_radians() / count as f64;
        let radii = rx.max(ry);
        let (nx, ny) = if radii > 0.0 {
            (rx / radii, ry / radii)
        } else {
            (1.0, 1.0)
        };
        let mut total = 0.0;
        for index in 0..=count {
            let (sin, cos) = (start + index as f64 * step).sin_cos();
            let angle = point(sin, -cos);
            self.vertices.push(point(rx * angle.x, ry * angle.y));
            self.normals.push(unit(point(ny * angle.x, nx * angle.y)));
            if index > 0 {
                // Compute chord lengths from the angle delta to avoid cancellation on tiny sweeps.
                let (mid_sin, mid_cos) = (start + (index as f64 - 0.5) * step).sin_cos();
                total += 2.0 * (step / 2.0).sin() * (nx * mid_cos).hypot(ny * mid_sin);
            }
            self.distances.push(total);
        }
        if self.closed {
            self.vertices[count] = self.vertices[0];
            self.normals[count] = self.normals[0];
        }
        // The radial layout allows radius zero. Its wave still traverses the authored angles.
        if radii > 0.0 && total * radii > 0.0 {
            self.distances
                .iter_mut()
                .for_each(|distance| *distance *= radii);
        }
    }

    fn length(&self) -> f64 {
        self.distances.last().copied().unwrap_or(0.0)
    }

    fn edge_normal(&self, index: usize) -> Point {
        let delta = subtract(self.vertices[index + 1], self.vertices[index]);
        unit(point(delta.y, -delta.x))
    }

    fn sample(&self, distance: f64) -> (Point, Point) {
        let end = self.distances.len() - 1;
        let index = self
            .distances
            .partition_point(|value| *value <= distance)
            .saturating_sub(1)
            .min(end - 1);
        let segment = self.distances[index + 1] - self.distances[index];
        let t = if segment > 0.0 {
            ((distance - self.distances[index]) / segment).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let p = add(
            self.origin,
            lerp(self.vertices[index], self.vertices[index + 1], t),
        );
        let normal = if !self.normals.is_empty() {
            unit(lerp(self.normals[index], self.normals[index + 1], t))
        } else if t == 0.0 && (index > 0 || self.closed) {
            let previous = if index > 0 { index - 1 } else { end - 1 };
            let next = self.edge_normal(index);
            let bisector = unit(add(self.edge_normal(previous), next));
            if bisector == Point::default() {
                next
            } else {
                bisector
            }
        } else if self.closed && t == 1.0 && index + 1 == end {
            return self.sample(0.0);
        } else {
            self.edge_normal(index)
        };
        (
            p,
            if self.reverse_normal {
                scale(normal, -1.0)
            } else {
                normal
            },
        )
    }

    fn ordered<'a>(
        &self,
        bands: &'a [f32],
    ) -> impl ExactSizeIterator<Item = (usize, f64)> + use<'a> {
        let order = self.order;
        let count = bands.len();
        let visual_count = if order == Spectrum2DBandMapping::CenterOut {
            count * 2
        } else {
            count
        };
        (0..visual_count).map(move |visual| {
            let index = match order {
                Spectrum2DBandMapping::Forward => visual,
                Spectrum2DBandMapping::Reverse => count - 1 - visual,
                Spectrum2DBandMapping::CenterOut if visual < count => count - 1 - visual,
                Spectrum2DBandMapping::CenterOut => visual - count,
            };
            (index, f64::from(bands[index]).clamp(0.0, 1.0))
        })
    }

    fn offset(&self, distance: f64, amplitude: f64) -> Point {
        let (base, normal) = self.sample(distance);
        add(base, scale(normal, self.reach * amplitude))
    }
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn add(a: Point, b: Point) -> Point {
    point(a.x + b.x, a.y + b.y)
}

fn subtract(a: Point, b: Point) -> Point {
    point(a.x - b.x, a.y - b.y)
}

fn scale(p: Point, by: f64) -> Point {
    point(p.x * by, p.y * by)
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    add(a, scale(subtract(b, a), t))
}

fn unit(p: Point) -> Point {
    let length = p.x.hypot(p.y);
    if length == 0.0 {
        Point::default()
    } else {
        point(p.x / length, p.y / length)
    }
}

fn skia_point(p: Point) -> tiny_skia::Point {
    tiny_skia::Point::from_xy(p.x as f32, p.y as f32)
}

fn append_points(builder: &mut PathBuilder, points: impl IntoIterator<Item = Point>) {
    let mut points = points.into_iter();
    if let Some(first) = points.next() {
        builder.move_to(first.x as f32, first.y as f32);
        for p in points {
            builder.line_to(p.x as f32, p.y as f32);
        }
    }
}

fn polygon(points: impl IntoIterator<Item = Point>) -> Option<Path> {
    let mut builder = PathBuilder::new();
    append_points(&mut builder, points);
    builder.close();
    builder.finish()
}

fn skia_color([r, g, b, a]: [u8; 4], opacity: f64) -> Color {
    Color::from_rgba8(r, g, b, (f64::from(a) * opacity).round() as u8)
}

fn paint(colour: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(colour);
    paint
}

fn mix_colour(start: [u8; 4], end: [u8; 4], t: f64) -> [u8; 4] {
    std::array::from_fn(|i| (f64::from(start[i]) * (1.0 - t) + f64::from(end[i]) * t).round() as u8)
}

#[expect(
    clippy::too_many_arguments,
    reason = "bar drawing uses the independent evaluated source controls"
)]
fn draw_bars(
    pixels: &mut Pixmap,
    path: &Baseline,
    bands: &[f32],
    gap: f64,
    minimum: f64,
    colour: [u8; 4],
    gradient: Option<Gradient>,
    opacity: f64,
) {
    if bands.is_empty() || opacity <= 0.0 || path.length() == 0.0 {
        return;
    }
    let levels = path.ordered(bands);
    let cell = path.length() / levels.len() as f64;
    let half_width = cell * (1.0 - gap) / 2.0;
    let max_index = bands.len() - 1;
    for (position, (index, level)) in levels.enumerate() {
        let level = minimum + level * (1.0 - minimum);
        if level <= 0.0 {
            continue;
        }
        let mid = (position as f64 + 0.5) * cell;
        let start = mid - half_width;
        let end = mid + half_width;
        let first = path.distances.partition_point(|d| *d <= start);
        let last = path.distances.partition_point(|d| *d < end);
        let distances = std::iter::once(start)
            .chain(path.distances[first..last.max(first)].iter().copied())
            .chain(std::iter::once(end));
        let bottom = if path.two_sided { -level } else { 0.0 };
        let Some(bar) = polygon(
            distances
                .clone()
                .map(|d| path.offset(d, level))
                .chain(distances.rev().map(|d| path.offset(d, bottom))),
        ) else {
            continue;
        };
        let mut paint = match gradient {
            Some((Spectrum2DGradientDirection::AcrossBands, start, end)) => {
                let t = if max_index == 0 {
                    0.5
                } else {
                    index as f64 / max_index as f64
                };
                paint(skia_color(mix_colour(start, end, t), opacity))
            }
            _ => paint(skia_color(colour, opacity)),
        };
        if let Some((Spectrum2DGradientDirection::AlongBar, start, end)) = gradient {
            let inner = skia_color(start, opacity);
            let outer = skia_color(end, opacity);
            let stops = if path.two_sided {
                vec![
                    GradientStop::new(0.0, outer),
                    GradientStop::new(0.5, inner),
                    GradientStop::new(1.0, outer),
                ]
            } else {
                vec![GradientStop::new(0.0, inner), GradientStop::new(1.0, outer)]
            };
            if let Some(shader) = LinearGradient::new(
                skia_point(path.offset(mid, if path.two_sided { -1.0 } else { 0.0 })),
                skia_point(path.offset(mid, 1.0)),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
            }
        }
        pixels.fill_path(&bar, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn amplitude(bands: &[f64], fraction: f64, closed: bool) -> f64 {
    let position = fraction * bands.len() as f64 - 0.5;
    let left = position.floor();
    let t = position - left;
    let band = |offset: isize| {
        let index = left as isize + offset;
        let n = bands.len() as isize;
        bands[if closed {
            index.rem_euclid(n)
        } else {
            index.clamp(0, n - 1)
        } as usize]
    };
    let (start, end) = (band(0), band(1));
    let delta = end - start;
    // Harmonic tangents preserve monotonicity between tips and stay flat at extrema.
    let tangent = |neighbor: f64| {
        if neighbor * delta > 0.0 {
            2.0 * neighbor * delta / (neighbor + delta)
        } else {
            0.0
        }
    };
    let before = tangent(start - band(-1));
    let after = tangent(band(2) - end);
    let complement = 1.0 - t;
    start + t * t * (3.0 - 2.0 * t) * delta + t * complement * (complement * before - t * after)
}

struct Wave {
    outline: Path,
    fill: Option<Path>,
    colour: [u8; 4],
    weight: f64,
}

fn draw_waves(
    pixels: &mut Pixmap,
    path: &Baseline,
    mixed: &[f32],
    source_colour: [u8; 4],
    wave: &EvaluatedSpectrum2DWave,
) {
    let settings = &wave.settings;
    let tint = settings
        .colour
        .as_deref()
        .and_then(parse_colour)
        .unwrap_or(source_colour);
    let channels = match (&wave.channels, &settings.stereo) {
        (Some([left, right]), Some(stereo)) => vec![
            (
                left.as_slice(),
                stereo
                    .left_colour
                    .as_deref()
                    .and_then(parse_colour)
                    .unwrap_or(tint),
                stereo.left_weight,
            ),
            (
                right.as_slice(),
                stereo
                    .right_colour
                    .as_deref()
                    .and_then(parse_colour)
                    .unwrap_or(tint),
                stereo.right_weight,
            ),
        ],
        _ => vec![(mixed, tint, 1.0)],
    };
    let mut waves = Vec::new();
    for (bands, colour, weight) in channels {
        if weight <= 0.0
            || colour[3] == 0
            || !bands.iter().any(|&band| band > 0.0)
            || path.length() <= 0.0
        {
            continue;
        }
        let levels: Vec<_> = path.ordered(bands).map(|(_, value)| value).collect();
        // Use an even number of steps per cell so every band center is a vertex.
        let subdivisions = (path.reach.sqrt().ceil() as usize).clamp(8, 128) * 2;
        let steps = levels.len() * subdivisions;
        let length = path.length();
        let mut distances: Vec<_> = (0..=steps)
            .map(|i| length * (i as f64 / steps as f64))
            .chain(path.distances.iter().copied())
            .collect();
        distances.sort_unstable_by(f64::total_cmp);
        distances.dedup();
        let sides = if path.two_sided {
            [1.0, -1.0].as_slice()
        } else {
            [1.0].as_slice()
        };
        for &side in sides {
            let tips: Vec<_> = distances
                .iter()
                .map(|&d| path.offset(d, side * amplitude(&levels, d / length, path.closed)))
                .collect();
            let mut outline = PathBuilder::new();
            append_points(&mut outline, tips.iter().copied());
            if path.closed {
                outline.close();
            }
            let Some(outline) = outline.finish() else {
                continue;
            };
            let fill = if settings.fill_opacity <= 0.0 {
                None
            } else if path.closed {
                let mut fill = PathBuilder::new();
                fill.push_path(&outline);
                append_points(&mut fill, distances.iter().rev().map(|&d| path.sample(d).0));
                fill.close();
                fill.finish()
            } else {
                polygon(
                    tips.into_iter()
                        .chain(distances.iter().rev().map(|&d| path.sample(d).0)),
                )
            };
            waves.push(Wave {
                outline,
                fill,
                colour,
                weight,
            });
        }
    }
    let stroke_width = settings.stroke_width * wave.pixel_scale;
    let radius = settings.glow_radius * wave.pixel_scale;
    for wave in &waves {
        if let Some(fill) = &wave.fill {
            pixels.fill_path(
                fill,
                &paint(skia_color(wave.colour, wave.weight * settings.fill_opacity)),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    for wave in &waves {
        if radius > 0.0 && settings.glow_opacity > 0.0 {
            feather_stroke(
                pixels,
                wave,
                stroke_width,
                radius,
                wave.weight * settings.glow_opacity,
            );
        }
    }
    for wave in &waves {
        stroke(
            pixels,
            &wave.outline,
            skia_color(wave.colour, wave.weight),
            stroke_width,
        );
    }
}

fn feather_stroke(pixels: &mut Pixmap, wave: &Wave, width: f64, radius: f64, opacity: f64) {
    // Draw increasingly opaque contours whose combined alpha falls smoothly across the halo.
    let rings = radius.ceil().clamp(1.0, 16.0) as u32;
    let maximum = opacity * f64::from(wave.colour[3]) / 255.0;
    let mut applied_alpha = 0.0;
    for ring in (1..=rings).rev() {
        let distance = (f64::from(ring) - 0.5) / f64::from(rings);
        let alpha = maximum * (1.0 - distance * distance).powi(3);
        let step_alpha = (alpha - applied_alpha) / (1.0 - applied_alpha);
        let [r, g, b, _] = wave.colour;
        stroke(
            pixels,
            &wave.outline,
            skia_color([r, g, b, 255], step_alpha),
            width + 2.0 * radius * f64::from(ring) / f64::from(rings),
        );
        applied_alpha = alpha;
    }
}

fn stroke(pixels: &mut Pixmap, path: &Path, colour: Color, width: f64) {
    pixels.stroke_path(
        path,
        &paint(colour),
        &Stroke {
            width: width as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        },
        Transform::identity(),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use vestra_core::project::{Spectrum2DEllipseLayout, Spectrum2DPathLayout, Spectrum2DWave};

    fn spectrum(layout: Spectrum2DLayout, bands: Vec<f32>) -> EvaluatedSource {
        EvaluatedSource::Spectrum2D {
            bands,
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
            bar_gap_ratio: 0.4,
            min_bar_height_ratio: 0.0,
            layout,
            gradient: None,
            colour: [255; 4],
            wave: None,
        }
    }

    #[test]
    fn arc_cells_follow_distance_in_pixels_and_normals_follow_the_ellipse() {
        let path = Baseline::new(
            &Spectrum2DLayout::Ellipse(Spectrum2DEllipseLayout {
                radius_x: 0.5,
                radius_y: 0.5,
                start_angle_degrees: 270.0,
                sweep_angle_degrees: 180.0,
                direction: Spectrum2DRadialDirection::Outward,
                band_mapping: Spectrum2DPathBandMapping::Forward,
            }),
            [0.0, 0.0, 200.0, 40.0],
        );
        let centers: Vec<_> = (0..12)
            .map(|index| path.sample(path.length() * (index as f64 + 0.5) / 12.0))
            .collect();
        assert!(centers.windows(2).all(|pair| {
            let distance = subtract(pair[1].0, pair[0].0);
            let ratio = distance.x.hypot(distance.y) / (path.length() / 12.0);
            (0.91..=1.01).contains(&ratio)
        }));
        let (apex, normal) = path.sample(path.length() / 2.0);
        assert!(subtract(apex, point(100.0, 0.0)).x.abs() < 1e-8);
        assert!(apex.y.abs() < 1e-8);
        assert!(normal.x.abs() < 1e-8 && normal.y < -0.999);
    }

    #[test]
    fn polyline_frequency_mapping_and_growth_follow_traversal() {
        let layout = |band_mapping| {
            Spectrum2DLayout::Path(Spectrum2DPathLayout {
                points: vec![point(0.0, 0.5), point(0.1, 0.5), point(1.0, 0.5)],
                closed: false,
                direction: Spectrum2DPathDirection::Left,
                band_mapping,
            })
        };
        let forward = rasterize(
            &spectrum(layout(Spectrum2DPathBandMapping::Forward), vec![0.5, 0.0]),
            100,
            100,
        )
        .unwrap();
        let reverse = rasterize(
            &spectrum(layout(Spectrum2DPathBandMapping::Reverse), vec![0.5, 0.0]),
            100,
            100,
        )
        .unwrap();
        assert!(forward.get_pixel(37, 35)[3] > 0);
        assert_eq!(forward.get_pixel(62, 35)[3], 0);
        assert!(reverse.get_pixel(62, 35)[3] > 0);
        assert_eq!(reverse.get_pixel(37, 35)[3], 0);
        assert_eq!(forward.get_pixel(37, 55)[3], 0);
    }

    #[test]
    fn polyline_corner_and_closed_seam_use_the_bisector() {
        let path = Baseline::new(
            &Spectrum2DLayout::Path(Spectrum2DPathLayout {
                points: vec![point(0.0, 0.0), point(1.0, 0.0), point(1.0, 1.0)],
                closed: true,
                direction: Spectrum2DPathDirection::Left,
                band_mapping: Spectrum2DPathBandMapping::Forward,
            }),
            [0.0, 0.0, 100.0, 100.0],
        );
        let (corner, normal) = path.sample(100.0);
        let diagonal = std::f64::consts::FRAC_1_SQRT_2;
        assert_eq!(corner, point(100.0, 0.0));
        assert!((normal.x - diagonal).abs() < 1e-12);
        assert!((normal.y + diagonal).abs() < 1e-12);
        assert_eq!(path.sample(50.0).1, point(0.0, -1.0));
        assert_eq!(path.sample(150.0).1, point(1.0, 0.0));
        assert_eq!(path.sample(0.0), path.sample(path.length()));
    }

    #[test]
    fn wave_has_periodic_seam_and_silence_is_transparent() {
        let bands = [0.0, 1.0, 0.5];
        assert_eq!(amplitude(&bands, 0.0, true), amplitude(&bands, 1.0, true));
        assert_eq!(amplitude(&bands, 0.0, false), 0.0);
        assert_eq!(amplitude(&bands, 1.0, false), 0.5);

        let mut source = spectrum(Spectrum2DLayout::default(), vec![0.0; 3]);
        let EvaluatedSource::Spectrum2D { wave, .. } = &mut source else {
            unreachable!();
        };
        *wave = Some(EvaluatedSpectrum2DWave {
            style: Spectrum2DStyle::Wave,
            settings: Spectrum2DWave::default().into(),
            channels: None,
            pixel_scale: 1.0,
        });
        assert!(
            rasterize(&source, 100, 100)
                .unwrap()
                .pixels()
                .all(|p| p[3] == 0)
        );
    }

    #[test]
    fn closed_ellipse_fill_and_glow_track_the_baseline_and_tip() {
        let mut source = spectrum(
            Spectrum2DLayout::Ellipse(Spectrum2DEllipseLayout {
                radius_x: 0.5,
                radius_y: 0.5,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: Spectrum2DRadialDirection::Outward,
                band_mapping: Spectrum2DPathBandMapping::Forward,
            }),
            vec![0.2; 8],
        );
        let EvaluatedSource::Spectrum2D { wave, .. } = &mut source else {
            unreachable!();
        };
        *wave = Some(EvaluatedSpectrum2DWave {
            style: Spectrum2DStyle::Wave,
            settings: Spectrum2DWave {
                glow_radius: 6.0,
                ..Default::default()
            }
            .into(),
            channels: None,
            pixel_scale: 1.0,
        });
        let rgba = rasterize(&source, 100, 100).unwrap();
        let alpha = |x, y| rgba.get_pixel(x, y)[3];
        assert_eq!(
            alpha(50, 50),
            0,
            "closed fill leaves the ellipse interior clear"
        );
        assert!(
            (30..=80).contains(&alpha(50, 23)),
            "fill reaches the top of the baseline"
        );
        assert!(
            alpha(50, 14) > 200,
            "stroke follows the tips across the seam"
        );
        assert!(
            (1..100).contains(&alpha(50, 11)),
            "halo reaches outside the stroke"
        );
        assert_eq!(alpha(50, 5), 0);
    }

    #[test]
    fn stereo_wave_draws_only_the_on_channel_and_preserves_its_tint() {
        use vestra_core::project::Spectrum2DStereo;

        for left_active in [true, false] {
            let mut source = spectrum(Spectrum2DLayout::default(), Vec::new());
            let EvaluatedSource::Spectrum2D { wave, .. } = &mut source else {
                unreachable!();
            };
            let levels = |audible| vec![if audible { 0.5 } else { 0.0 }; 3];
            *wave = Some(EvaluatedSpectrum2DWave {
                style: Spectrum2DStyle::Wave,
                settings: Spectrum2DWave {
                    stereo: Some(Spectrum2DStereo {
                        left_colour: Some("#ff0000".into()),
                        right_colour: Some("#0000ff".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
                .into(),
                channels: Some([levels(left_active), levels(!left_active)]),
                pixel_scale: 1.0,
            });
            let rgba = rasterize(&source, 100, 100).unwrap();
            let stroke = rgba.get_pixel(50, 49);
            let (expected, maximum_alpha) = if left_active {
                ([255, 0, 0], 166)
            } else {
                ([0, 0, 255], 255)
            };
            assert_eq!(stroke.0[..3], expected);
            assert_eq!(stroke[3], maximum_alpha);
            assert!(
                rgba.pixels()
                    .filter(|p| p[3] != 0)
                    .all(|p| p.0[..3] == expected)
            );
        }
    }
}
