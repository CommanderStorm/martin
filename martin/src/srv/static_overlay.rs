use std::f64::consts::PI;
use std::sync::LazyLock;

use geo_types::Coord;
use image::{Rgba, RgbaImage};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Parse a CSS color string into an anti-aliased `tiny_skia::Paint`.
///
/// Supports all CSS color formats via `csscolorparser`: named colors, hex,
/// rgb(), rgba(), hsl(), hsla(), hwb(), lab(), lch(), and more.
#[must_use]
pub fn parse_paint(s: &str) -> Option<Paint<'static>> {
    let css: csscolorparser::Color = s.trim().parse().ok()?;
    let [r, g, b, a] = css.to_rgba8();
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, a);
    paint.anti_alias = true;
    paint.force_hq_pipeline = cfg!(debug_assertions);
    Some(paint)
}

/// A path overlay to draw on the map.
#[derive(Debug, Clone)]
pub struct PathOverlay {
    pub points: Vec<Coord>,
    pub stroke: Option<Paint<'static>>,
    pub fill: Option<Paint<'static>>,
    pub width: Option<f32>,
}

/// A marker overlay to draw on the map.
#[derive(Debug, Clone)]
pub struct MarkerOverlay {
    pub coord: Coord,
    pub icon_path: String,
    pub scale: f32,
    pub offset: (f32, f32),
}

/// Default styling for overlays from query parameters.
#[derive(Debug, Clone, Default)]
pub struct OverlayDefaults {
    pub fill: Option<Paint<'static>>,
    pub stroke: Option<Paint<'static>>,
    pub width: Option<f32>,
    pub stroke_style: Stroke,
    pub border: Option<Paint<'static>>,
    pub borderwidth: Option<f32>,
}

/// Parse a line cap string into a `tiny_skia::LineCap`.
#[must_use]
pub fn parse_linecap(s: &str) -> tiny_skia::LineCap {
    match s.to_ascii_lowercase().as_str() {
        "butt" => tiny_skia::LineCap::Butt,
        "square" => tiny_skia::LineCap::Square,
        _ => tiny_skia::LineCap::Round,
    }
}

/// Parse a line join string into a `tiny_skia::LineJoin`.
#[must_use]
pub fn parse_linejoin(s: &str) -> tiny_skia::LineJoin {
    match s.to_ascii_lowercase().as_str() {
        "miter" => tiny_skia::LineJoin::Miter,
        "bevel" => tiny_skia::LineJoin::Bevel,
        _ => tiny_skia::LineJoin::Round,
    }
}

// ── Parsing ──────────────────────────────────────────────────────

/// Parse a `path` query parameter.
///
/// Format: `(option:|)*coords` where options are `fill:color`, `stroke:color`, `width:N`
/// and coords are `lng,lat|lng,lat|...` or `enc:polyline`.
#[must_use]
pub fn parse_path(s: &str, latlng: bool) -> Option<PathOverlay> {
    let mut stroke = None;
    let mut fill = None;
    let mut width = None;
    let mut coord_part = s;

    // Parse prefix options separated by `|`
    // Options are key:value pairs; coordinates are numbers
    // Walk from left, consuming recognized options until we hit coords
    loop {
        if let Some((segment, rest)) = coord_part.split_once('|') {
            if let Some(val) = segment.strip_prefix("fill:") {
                fill = parse_paint(val);
                coord_part = rest;
            } else if let Some(val) = segment.strip_prefix("stroke:") {
                stroke = parse_paint(val);
                coord_part = rest;
            } else if let Some(val) = segment.strip_prefix("width:") {
                width = val.parse::<f32>().ok();
                coord_part = rest;
            } else {
                // Not an option, this is the start of coordinates
                break;
            }
        } else {
            break;
        }
    }

    let points = parse_coords(coord_part, latlng)?;
    if points.len() < 2 {
        return None;
    }

    Some(PathOverlay {
        points,
        stroke,
        fill,
        width,
    })
}

/// Parse coordinates from string: raw `lng,lat|lng,lat` or `enc:polyline`.
fn parse_coords(s: &str, latlng: bool) -> Option<Vec<Coord>> {
    if let Some(encoded) = s.strip_prefix("enc:") {
        let coords: Vec<Coord> = polyline::decode_polyline(encoded, 5)
            .ok()?
            .into_iter()
            .collect();
        return if coords.is_empty() { None } else { Some(coords) };
    }

    let parts: Vec<&str> = s.split('|').collect();
    let mut coords = Vec::with_capacity(parts.len());
    for part in parts {
        let (a, b) = part.split_once(',')?;
        let a: f64 = a.trim().parse().ok()?;
        let b: f64 = b.trim().parse().ok()?;
        if latlng {
            coords.push(Coord { x: b, y: a });
        } else {
            coords.push(Coord { x: a, y: b });
        }
    }

    if coords.is_empty() {
        None
    } else {
        Some(coords)
    }
}

/// Parse a `marker` query parameter.
///
/// Format: `lng,lat|iconPath[|option:value]*`
#[must_use]
pub fn parse_marker(s: &str, latlng: bool) -> Option<MarkerOverlay> {
    let mut parts = s.splitn(3, '|');
    let coord_str = parts.next()?;
    let icon_path = parts.next()?.to_string();
    if icon_path.is_empty() {
        return None;
    }

    let (a, b) = coord_str.split_once(',')?;
    let a: f64 = a.trim().parse().ok()?;
    let b: f64 = b.trim().parse().ok()?;
    let coord = if latlng {
        Coord { x: b, y: a }
    } else {
        Coord { x: a, y: b }
    };

    let mut scale: f32 = 1.0;
    let mut offset: (f32, f32) = (0.0, 0.0);

    if let Some(options_str) = parts.next() {
        for opt in options_str.split('|') {
            if let Some(val) = opt.strip_prefix("scale:") {
                if let Ok(v) = val.parse::<f32>() {
                    scale = v;
                }
            } else if let Some(val) = opt.strip_prefix("offset:") {
                if let Some((x, y)) = val.split_once(',') {
                    if let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) {
                        offset = (x, y);
                    }
                }
            }
        }
    }

    Some(MarkerOverlay {
        coord,
        icon_path,
        scale,
        offset,
    })
}

// ── Geo/pixel conversion ─────────────────────────────────────────

/// Convert geographic coordinates to pixel position in the image.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub fn geo_to_pixel(
    lon: f64,
    lat: f64,
    zoom: f64,
    img_width: u32,
    img_height: u32,
    center_lon: f64,
    center_lat: f64,
) -> (f64, f64) {
    let scale = 256.0 * 2_f64.powf(zoom);

    let world_x = (lon + 180.0) / 360.0 * scale;
    let world_y =
        (1.0 - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / PI) / 2.0 * scale;

    let center_world_x = (center_lon + 180.0) / 360.0 * scale;
    let center_world_y = (1.0
        - (center_lat.to_radians().tan() + 1.0 / center_lat.to_radians().cos()).ln() / PI)
        / 2.0
        * scale;

    let px_x = world_x - center_world_x + f64::from(img_width) / 2.0;
    let px_y = world_y - center_world_y + f64::from(img_height) / 2.0;

    (px_x, px_y)
}

/// Convert pixel position back to geographic coordinates.
#[cfg(test)]
#[must_use]
pub fn pixel_to_geo(
    px_x: f64,
    px_y: f64,
    zoom: f64,
    img_width: u32,
    img_height: u32,
    center_lon: f64,
    center_lat: f64,
) -> (f64, f64) {
    let scale = 256.0 * 2_f64.powf(zoom);

    let center_world_x = (center_lon + 180.0) / 360.0 * scale;
    let center_world_y = (1.0
        - (center_lat.to_radians().tan() + 1.0 / center_lat.to_radians().cos()).ln() / PI)
        / 2.0
        * scale;

    let world_x = px_x - f64::from(img_width) / 2.0 + center_world_x;
    let world_y = px_y - f64::from(img_height) / 2.0 + center_world_y;

    let lon = world_x / scale * 360.0 - 180.0;
    let lat_rad = (PI * (1.0 - 2.0 * world_y / scale)).sinh().atan();
    let lat = lat_rad.to_degrees();

    (lon, lat)
}

// ── Bounds calculation ───────────────────────────────────────────

/// Calculate bounding box from overlays: `(min_lon, min_lat, max_lon, max_lat)`.
#[must_use]
pub fn calculate_bounds(
    paths: &[PathOverlay],
    markers: &[MarkerOverlay],
) -> Option<(f64, f64, f64, f64)> {
    let all_coords = paths
        .iter()
        .flat_map(|p| p.points.iter())
        .chain(markers.iter().map(|m| &m.coord));

    let mut min_lon = f64::MAX;
    let mut min_lat = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut max_lat = f64::MIN;
    let mut count = 0;

    for c in all_coords {
        min_lon = min_lon.min(c.x);
        min_lat = min_lat.min(c.y);
        max_lon = max_lon.max(c.x);
        max_lat = max_lat.max(c.y);
        count += 1;
    }

    if count == 0 {
        None
    } else {
        Some((min_lon, min_lat, max_lon, max_lat))
    }
}

/// Calculate center and zoom from a bounding box.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn bbox_to_center_zoom(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    width: u32,
    height: u32,
    padding: f64,
    max_zoom: Option<f64>,
) -> (f64, f64, f64) {
    let center_lon = (min_lon + max_lon) / 2.0;
    let center_lat = (min_lat + max_lat) / 2.0;

    let lon_range = (max_lon - min_lon) * (1.0 + 2.0 * padding);
    let lat_range = (max_lat - min_lat) * (1.0 + 2.0 * padding);

    // Handle single-point case
    if lon_range.abs() < 1e-10 && lat_range.abs() < 1e-10 {
        let zoom = max_zoom.unwrap_or(14.0).min(14.0);
        return (center_lon, center_lat, zoom);
    }

    // Calculate zoom from lon/lat range and image dimensions
    let zoom_lon = if lon_range > 0.0 {
        (360.0 * f64::from(width) / (256.0 * lon_range)).log2()
    } else {
        20.0
    };

    let zoom_lat = if lat_range > 0.0 {
        // Use Mercator projection for latitude
        let lat_rad_min = (min_lat - lat_range * padding / (1.0 + 2.0 * padding)).to_radians();
        let lat_rad_max = (max_lat + lat_range * padding / (1.0 + 2.0 * padding)).to_radians();
        let merc_range =
            (lat_rad_max.tan() + 1.0 / lat_rad_max.cos()).ln()
                - (lat_rad_min.tan() + 1.0 / lat_rad_min.cos()).ln();
        if merc_range.abs() > 1e-10 {
            (2.0 * PI * f64::from(height) / (256.0 * merc_range)).log2()
        } else {
            20.0
        }
    } else {
        20.0
    };

    let mut zoom = zoom_lon.min(zoom_lat).max(0.0);
    if let Some(mz) = max_zoom {
        zoom = zoom.min(mz);
    }

    (center_lon, center_lat, zoom)
}

// ── Overlay drawing (tiny-skia) ──────────────────────────────────

/// Convert an `RgbaImage` to a `tiny_skia::Pixmap` for drawing, then back.
/// tiny-skia uses premultiplied alpha RGBA internally.
fn rgba_to_pixmap(img: &RgbaImage) -> Option<Pixmap> {
    let (w, h) = (img.width(), img.height());
    let mut pixmap = Pixmap::new(w, h)?;
    for (x, y, pixel) in img.enumerate_pixels() {
        let c = tiny_skia::ColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3])
            .premultiply();
        pixmap.pixels_mut()[(y * w + x) as usize] = c;
    }
    Some(pixmap)
}

fn pixmap_to_rgba(pixmap: &Pixmap) -> RgbaImage {
    let w = pixmap.width();
    let h = pixmap.height();
    let mut img = RgbaImage::new(w, h);
    for (i, px) in pixmap.pixels().iter().enumerate() {
        let c = px.demultiply();
        let x = (i as u32) % w;
        let y = (i as u32) / w;
        img.put_pixel(x, y, Rgba([c.red(), c.green(), c.blue(), c.alpha()]));
    }
    img
}

/// Build a tiny-skia path from pixel coordinates, optionally closed for fills.
fn build_path(pixels: &[(f64, f64)], closed: bool) -> Option<tiny_skia::Path> {
    if pixels.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::new();
    #[allow(clippy::cast_possible_truncation)]
    {
        pb.move_to(pixels[0].0 as f32, pixels[0].1 as f32);
        for &(x, y) in &pixels[1..] {
            pb.line_to(x as f32, y as f32);
        }
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

/// Draw overlays onto a rendered image using tiny-skia for anti-aliased rendering.
pub fn draw_overlays(
    img: &mut RgbaImage,
    paths: &[PathOverlay],
    markers: &[MarkerOverlay],
    center_lon: f64,
    center_lat: f64,
    zoom: f64,
    defaults: &OverlayDefaults,
) {
    static DEFAULT_BLACK: LazyLock<Paint<'static>> =
        LazyLock::new(|| parse_paint("black").expect("black is a valid CSS color"));

    let (w, h) = (img.width(), img.height());
    let Some(mut pixmap) = rgba_to_pixmap(img) else {
        return;
    };
    let identity = Transform::identity();
    let mut pixels: Vec<(f64, f64)> = Vec::new();

    for path in paths {
        pixels.clear();
        pixels.extend(
            path.points
                .iter()
                .map(|c| geo_to_pixel(c.x, c.y, zoom, w, h, center_lon, center_lat)),
        );

        let stroke_paint = path.stroke.as_ref().or(defaults.stroke.as_ref());
        let fill_paint = path.fill.as_ref().or(defaults.fill.as_ref());
        let stroke_width = path.width.or(defaults.width).unwrap_or(3.0);

        // Fill (closed polygon)
        if let Some(fill) = fill_paint {
            if pixels.len() >= 3 {
                if let Some(skia_path) = build_path(&pixels, true) {
                    pixmap.fill_path(&skia_path, fill, FillRule::EvenOdd, identity, None);
                }
            }
        }

        // Border (halo) — wider stroke behind the main stroke
        if let Some(border_paint) = &defaults.border {
            let border_width =
                defaults.borderwidth.unwrap_or(stroke_width * 0.1) + stroke_width;
            if let Some(skia_path) = build_path(&pixels, false) {
                pixmap.stroke_path(
                    &skia_path,
                    border_paint,
                    &{ let mut s = defaults.stroke_style.clone(); s.width = border_width; s },
                    identity,
                    None,
                );
            }
        }

        // Stroke (explicit stroke, or default black when neither fill nor stroke)
        let effective_stroke = stroke_paint.or(if fill_paint.is_none() {
            Some(&DEFAULT_BLACK)
        } else {
            None
        });
        if let Some(stroke) = effective_stroke {
            if let Some(skia_path) = build_path(&pixels, false) {
                pixmap.stroke_path(
                    &skia_path,
                    stroke,
                    &{ let mut s = defaults.stroke_style.clone(); s.width = stroke_width; s },
                    identity,
                    None,
                );
            }
        }
    }

    // Markers
    for marker in markers {
        let (px, py) = geo_to_pixel(
            marker.coord.x,
            marker.coord.y,
            zoom,
            w,
            h,
            center_lon,
            center_lat,
        );

        if let Ok(icon) = image::open(&marker.icon_path) {
            let icon = icon.to_rgba8();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let scaled_w = (f64::from(icon.width()) * f64::from(marker.scale)).round() as u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let scaled_h = (f64::from(icon.height()) * f64::from(marker.scale)).round() as u32;

            let scaled = image::imageops::resize(
                &icon,
                scaled_w,
                scaled_h,
                image::imageops::FilterType::Lanczos3,
            );

            // Composite via tiny-skia
            if let Some(icon_pixmap) = rgba_to_pixmap(&scaled) {
                #[allow(clippy::cast_possible_truncation)]
                let dx =
                    (px + f64::from(marker.offset.0) - f64::from(scaled_w) / 2.0).round() as i32;
                #[allow(clippy::cast_possible_truncation)]
                let dy =
                    (py + f64::from(marker.offset.1) - f64::from(scaled_h) / 2.0).round() as i32;

                let paint = tiny_skia::PixmapPaint::default();
                pixmap.draw_pixmap(dx, dy, icon_pixmap.as_ref(), &paint, identity, None);
            }
        } else {
            // Fallback: anti-aliased circle marker
            static MARKER_BORDER: LazyLock<Paint<'static>> =
                LazyLock::new(|| parse_paint("rgba(205,0,0,0.784)").expect("valid CSS color"));
            static MARKER_FILL: LazyLock<Paint<'static>> =
                LazyLock::new(|| parse_paint("rgba(255,0,0,0.784)").expect("valid CSS color"));

            #[allow(clippy::cast_possible_truncation)]
            let (cx, cy, r) = (px as f32, py as f32, 8.0_f32);
            if let Some(circle) = PathBuilder::from_circle(cx, cy, r) {
                let mut border_stroke = Stroke::default();
                border_stroke.width = 2.0;
                pixmap.stroke_path(&circle, &MARKER_BORDER, &border_stroke, identity, None);
                pixmap.fill_path(&circle, &MARKER_FILL, FillRule::Winding, identity, None);
            }
        }
    }

    // Write pixmap back to RgbaImage
    *img = pixmap_to_rgba(&pixmap);
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::hex6("#ff0000")]
    #[case::hex8("#00ff0080")]
    #[case::hex3("#f00")]
    #[case::hex4("#f008")]
    #[case::named_red("red")]
    #[case::named_blue("Blue")]
    #[case::named_transparent("transparent")]
    #[case::named_green("green")]
    #[case::named_white("white")]
    #[case::named_black("black")]
    #[case::named_yellow("yellow")]
    #[case::named_cyan("cyan")]
    #[case::named_aqua("aqua")]
    #[case::named_magenta("magenta")]
    #[case::named_fuchsia("fuchsia")]
    #[case::named_orange("orange")]
    #[case::named_gray("gray")]
    #[case::named_grey("grey")]
    #[case::rgb("rgb(100,200,50)")]
    #[case::rgba_float("rgba(128,64,32,0.5)")]
    #[case::rgba_one("rgba(100,200,50,1)")]
    fn parse_paint_valid(#[case] input: &str) {
        assert!(parse_paint(input).is_some(), "{input:?} should parse");
    }

    #[rstest]
    #[case::unknown("notacolor")]
    #[case::bad_hex("#gg0000")]
    #[case::empty("")]
    fn parse_paint_invalid(#[case] input: &str) {
        assert!(parse_paint(input).is_none(), "{input:?} should not parse");
    }

    #[test]
    fn parse_path_raw_coords() {
        let path = parse_path("5.9,45.8|10.5,45.8|10.5,47.8", false).unwrap();
        assert_eq!(path.points.len(), 3);
        assert!((path.points[0].x - 5.9).abs() < 1e-10);
        assert!((path.points[0].y - 45.8).abs() < 1e-10);
        assert!(path.stroke.is_none());
        assert!(path.fill.is_none());
    }

    #[test]
    fn parse_path_with_options() {
        let path =
            parse_path("fill:red|stroke:#0000ff|width:5|5.9,45.8|10.5,47.8", false).unwrap();
        assert!(path.fill.is_some());
        assert!(path.stroke.is_some());
        assert_eq!(path.width, Some(5.0));
        assert_eq!(path.points.len(), 2);
    }

    #[test]
    fn parse_path_latlng_mode() {
        let path = parse_path("45.8,5.9|47.8,10.5", true).unwrap();
        assert!((path.points[0].y - 45.8).abs() < 1e-10);
        assert!((path.points[0].x - 5.9).abs() < 1e-10);
    }

    #[test]
    fn parse_path_encoded_polyline() {
        let path = parse_path("enc:_p~iF~ps|U_ulLnnqC_mqNvxq`@", false).unwrap();
        assert!(path.points.len() >= 2);
    }

    #[rstest]
    #[case::too_few_points("5.9,45.8")]
    #[case::invalid_coords("not,valid|coords")]
    #[case::only_options("fill:red|stroke:blue")]
    fn parse_path_rejected(#[case] input: &str) {
        assert!(parse_path(input, false).is_none());
    }

    #[test]
    fn parse_path_single_option_and_coords() {
        let path = parse_path("stroke:green|1.0,2.0|3.0,4.0", false).unwrap();
        assert!(path.stroke.is_some());
        assert!(path.fill.is_none());
        assert!(path.width.is_none());
        assert_eq!(path.points.len(), 2);
    }

    #[test]
    fn parse_marker_basic() {
        let marker = parse_marker("-122.4,37.8|/path/to/icon.png", false).unwrap();
        assert!((marker.coord.x - (-122.4)).abs() < 1e-10);
        assert!((marker.coord.y - 37.8).abs() < 1e-10);
        assert_eq!(marker.icon_path, "/path/to/icon.png");
        assert!((marker.scale - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_marker_with_options() {
        let marker = parse_marker("-122.4,37.8|icon.png|scale:2|offset:5,-3", false).unwrap();
        assert!((marker.scale - 2.0).abs() < f32::EPSILON);
        assert!((marker.offset.0 - 5.0).abs() < f32::EPSILON);
        assert!((marker.offset.1 - (-3.0)).abs() < f32::EPSILON);
    }

    #[rstest]
    #[case::no_pipe("just_text")]
    #[case::empty_icon("-122.4,37.8|")]
    #[case::no_icon("-122.4,37.8")]
    fn parse_marker_rejected(#[case] input: &str) {
        assert!(parse_marker(input, false).is_none());
    }

    #[test]
    fn parse_marker_latlng_mode() {
        let marker = parse_marker("37.8,-122.4|icon.png", true).unwrap();
        assert!((marker.coord.y - 37.8).abs() < 1e-10);
        assert!((marker.coord.x - (-122.4)).abs() < 1e-10);
    }

    #[test]
    fn parse_marker_scale_only() {
        let marker = parse_marker("10,20|icon.png|scale:0.5", false).unwrap();
        assert!((marker.scale - 0.5).abs() < f32::EPSILON);
        assert!((marker.offset.0).abs() < f32::EPSILON);
        assert!((marker.offset.1).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_marker_offset_only() {
        let marker = parse_marker("10,20|icon.png|offset:3,7", false).unwrap();
        assert!((marker.scale - 1.0).abs() < f32::EPSILON);
        assert!((marker.offset.0 - 3.0).abs() < f32::EPSILON);
        assert!((marker.offset.1 - 7.0).abs() < f32::EPSILON);
    }

    #[test]
    fn geo_to_pixel_center_is_center() {
        let (px, py) = geo_to_pixel(0.0, 0.0, 1.0, 512, 512, 0.0, 0.0);
        assert!((px - 256.0).abs() < 1.0);
        assert!((py - 256.0).abs() < 1.0);
    }

    #[test]
    fn geo_to_pixel_offset_from_center() {
        let (px, _) = geo_to_pixel(1.0, 0.0, 1.0, 512, 512, 0.0, 0.0);
        assert!(
            px > 256.0,
            "point east of center should be right of center pixel"
        );
    }

    #[test]
    fn pixel_to_geo_roundtrip() {
        let center_lon = -122.4;
        let center_lat = 37.8;
        let zoom = 10.0;

        let (px, py) = geo_to_pixel(-122.0, 37.5, zoom, 800, 600, center_lon, center_lat);
        let (lon, lat) = pixel_to_geo(px, py, zoom, 800, 600, center_lon, center_lat);
        assert!((lon - (-122.0)).abs() < 0.001);
        assert!((lat - 37.5).abs() < 0.001);
    }

    #[test]
    fn calculate_bounds_from_paths() {
        let paths = vec![PathOverlay {
            points: vec![
                Coord { x: 5.9, y: 45.8 },
                Coord { x: 10.5, y: 47.8 },
            ],
            stroke: None,
            fill: None,
            width: None,
        }];
        let bounds = calculate_bounds(&paths, &[]).unwrap();
        assert!((bounds.0 - 5.9).abs() < 1e-10);
        assert!((bounds.1 - 45.8).abs() < 1e-10);
        assert!((bounds.2 - 10.5).abs() < 1e-10);
        assert!((bounds.3 - 47.8).abs() < 1e-10);
    }

    #[test]
    fn calculate_bounds_empty() {
        assert!(calculate_bounds(&[], &[]).is_none());
    }

    #[test]
    fn calculate_bounds_from_markers() {
        let markers = vec![
            MarkerOverlay {
                coord: Coord { x: 1.0, y: 2.0 },
                icon_path: "icon.png".to_string(),
                scale: 1.0,
                offset: (0.0, 0.0),
            },
            MarkerOverlay {
                coord: Coord { x: 10.0, y: 20.0 },
                icon_path: "icon.png".to_string(),
                scale: 1.0,
                offset: (0.0, 0.0),
            },
        ];
        let bounds = calculate_bounds(&[], &markers).unwrap();
        assert!((bounds.0 - 1.0).abs() < 1e-10);
        assert!((bounds.1 - 2.0).abs() < 1e-10);
        assert!((bounds.2 - 10.0).abs() < 1e-10);
        assert!((bounds.3 - 20.0).abs() < 1e-10);
    }

    #[test]
    fn calculate_bounds_mixed_paths_and_markers() {
        let paths = vec![PathOverlay {
            points: vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 5.0, y: 5.0 },
            ],
            stroke: None,
            fill: None,
            width: None,
        }];
        let markers = vec![MarkerOverlay {
            coord: Coord { x: 10.0, y: -2.0 },
            icon_path: "icon.png".to_string(),
            scale: 1.0,
            offset: (0.0, 0.0),
        }];
        let bounds = calculate_bounds(&paths, &markers).unwrap();
        assert!((bounds.0 - 0.0).abs() < 1e-10);
        assert!((bounds.1 - (-2.0)).abs() < 1e-10);
        assert!((bounds.2 - 10.0).abs() < 1e-10);
        assert!((bounds.3 - 5.0).abs() < 1e-10);
    }

    #[test]
    fn bbox_to_center_zoom_basic() {
        let (clon, clat, zoom) =
            bbox_to_center_zoom(5.9, 45.8, 10.5, 47.8, 800, 600, 0.0, None);
        assert!((clon - 8.2).abs() < 0.01);
        assert!((clat - 46.8).abs() < 0.01);
        assert!(zoom > 0.0);
    }

    #[test]
    fn bbox_to_center_zoom_single_point() {
        let (clon, clat, zoom) =
            bbox_to_center_zoom(10.0, 45.0, 10.0, 45.0, 800, 600, 0.1, None);
        assert!((clon - 10.0).abs() < 1e-10);
        assert!((clat - 45.0).abs() < 1e-10);
        assert!((zoom - 14.0).abs() < 1e-10);
    }

    #[test]
    fn bbox_to_center_zoom_with_maxzoom() {
        let (_, _, zoom) = bbox_to_center_zoom(5.9, 45.8, 10.5, 47.8, 800, 600, 0.0, Some(3.0));
        assert!(
            zoom <= 3.0,
            "zoom {zoom} should be clamped to maxzoom 3.0"
        );
    }

    #[test]
    fn bbox_to_center_zoom_with_padding() {
        let (_, _, zoom_no_pad) =
            bbox_to_center_zoom(5.9, 45.8, 10.5, 47.8, 800, 600, 0.0, None);
        let (_, _, zoom_pad) =
            bbox_to_center_zoom(5.9, 45.8, 10.5, 47.8, 800, 600, 0.2, None);
        assert!(
            zoom_pad < zoom_no_pad,
            "padding should result in lower zoom: {zoom_pad} vs {zoom_no_pad}"
        );
    }

    #[test]
    fn linecap_parse() {
        assert_eq!(parse_linecap("butt"), tiny_skia::LineCap::Butt);
        assert_eq!(parse_linecap("round"), tiny_skia::LineCap::Round);
        assert_eq!(parse_linecap("square"), tiny_skia::LineCap::Square);
        assert_eq!(parse_linecap("ROUND"), tiny_skia::LineCap::Round);
        assert_eq!(parse_linecap("unknown"), tiny_skia::LineCap::Round);
    }

    #[test]
    fn linejoin_parse() {
        assert_eq!(parse_linejoin("miter"), tiny_skia::LineJoin::Miter);
        assert_eq!(parse_linejoin("round"), tiny_skia::LineJoin::Round);
        assert_eq!(parse_linejoin("bevel"), tiny_skia::LineJoin::Bevel);
        assert_eq!(parse_linejoin("BEVEL"), tiny_skia::LineJoin::Bevel);
        assert_eq!(parse_linejoin("unknown"), tiny_skia::LineJoin::Round);
    }
    
    mod render_tests {
        use std::io::Cursor;
        use std::path::Path;

        use super::*;

        const REF_DIR: &str = "../tests/fixtures/overlay_references";

        /// Compare a rendered image against a reference PNG.
        /// If no reference exists, create it and panic (so CI catches new tests).
        fn assert_image_snapshot(name: &str, img: &RgbaImage) {
            let ref_path = Path::new(REF_DIR).join(format!("{name}.png"));

            let mut rendered_png = Cursor::new(Vec::new());
            img.write_to(&mut rendered_png, image::ImageFormat::Png)
                .unwrap();

            if ref_path.exists() {
                let reference_png = std::fs::read(&ref_path).unwrap_or_else(|e| {
                    panic!("Failed to read reference {ref_path:?}: {e}")
                });

                let diff_pixels = pixelmatch::pixelmatch(
                    Cursor::new(&reference_png),
                    Cursor::new(rendered_png.get_ref()),
                    None::<&mut std::io::Sink>,
                    Some(img.width()),
                    Some(img.height()),
                    Some(pixelmatch::Options {
                        threshold: 0.1,
                        ..Default::default()
                    }),
                )
                .unwrap_or_else(|e| panic!("pixelmatch failed for {name}: {e}"));

                let total = img.width() * img.height();
                #[allow(clippy::cast_precision_loss)]
                let diff_pct = (diff_pixels as f64 / total as f64) * 100.0;
                assert!(
                    diff_pct < 1.0,
                    "Overlay render {name} differs by {diff_pct:.2}% ({diff_pixels} pixels). \
                     If expected, delete the reference and re-run."
                );
            } else {
                std::fs::create_dir_all(ref_path.parent().unwrap()).unwrap();
                img.write_to(
                    &mut std::io::BufWriter::new(std::fs::File::create(&ref_path).unwrap()),
                    image::ImageFormat::Png,
                )
                .unwrap();
                panic!(
                    "No reference at {ref_path:?}. Created from current render. \
                     Commit and re-run."
                );
            }
        }

        fn blank_canvas(w: u32, h: u32) -> RgbaImage {
            RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))
        }

        /// Helper: draw paths via draw_overlays using pixel coords mapped to geo
        /// by placing them at zoom 0 around (0,0) center.
        /// At zoom 0, the world is 256px, so pixel offsets map roughly 1:1
        /// for a 200x200 canvas centered at 0,0.
        fn draw_test_paths(
            img: &mut RgbaImage,
            paths: &[PathOverlay],
            defaults: &OverlayDefaults,
        ) {
            draw_overlays(img, paths, &[], 0.0, 0.0, 0.0, defaults);
        }

        /// Convert pixel coordinates (on a 200x200 canvas) to geo coords
        /// that will land at those pixel positions at zoom=0, center=(0,0).
        fn px_to_geo(px: f64, py: f64, w: u32, h: u32) -> Coord {
            let (x, y) = pixel_to_geo(px, py, 0.0, w, h, 0.0, 0.0);
            Coord { x, y }
        }

        #[test]
        fn render_path_stroke() {
            let mut img = blank_canvas(200, 200);
            let paths = vec![PathOverlay {
                points: vec![
                    px_to_geo(20.0, 100.0, 200, 200),
                    px_to_geo(100.0, 30.0, 200, 200),
                    px_to_geo(180.0, 100.0, 200, 200),
                ],
                stroke: parse_paint("rgb(0,0,200)"),
                fill: None,
                width: Some(4.0),
            }];
            draw_test_paths(&mut img, &paths, &OverlayDefaults::default());
            assert_image_snapshot("path_stroke", &img);
        }

        #[test]
        fn render_path_thick_stroke() {
            let mut img = blank_canvas(200, 200);
            let paths = vec![PathOverlay {
                points: vec![
                    px_to_geo(20.0, 150.0, 200, 200),
                    px_to_geo(100.0, 50.0, 200, 200),
                    px_to_geo(180.0, 150.0, 200, 200),
                ],
                stroke: parse_paint("rgb(200,0,0)"),
                fill: None,
                width: Some(12.0),
            }];
            draw_test_paths(&mut img, &paths, &OverlayDefaults::default());
            assert_image_snapshot("path_thick_stroke", &img);
        }

        #[test]
        fn render_polygon_fill() {
            let mut img = blank_canvas(200, 200);
            let paths = vec![PathOverlay {
                points: vec![
                    px_to_geo(30.0, 170.0, 200, 200),
                    px_to_geo(100.0, 20.0, 200, 200),
                    px_to_geo(170.0, 170.0, 200, 200),
                ],
                stroke: None,
                fill: parse_paint("rgba(0,150,0,0.706)"),
                width: None,
            }];
            draw_test_paths(&mut img, &paths, &OverlayDefaults::default());
            assert_image_snapshot("polygon_fill", &img);
        }

        #[test]
        fn render_path_with_border() {
            let mut img = blank_canvas(200, 200);
            let paths = vec![PathOverlay {
                points: vec![
                    px_to_geo(20.0, 100.0, 200, 200),
                    px_to_geo(180.0, 100.0, 200, 200),
                ],
                stroke: parse_paint("rgb(255,100,0)"),
                fill: None,
                width: Some(4.0),
            }];
            let defaults = OverlayDefaults {
                border: parse_paint("black"),
                borderwidth: Some(3.0),
                ..Default::default()
            };
            draw_test_paths(&mut img, &paths, &defaults);
            assert_image_snapshot("path_with_border", &img);
        }

        #[test]
        fn render_circle_marker() {
            let mut img = blank_canvas(200, 200);
            // Use fallback markers (no icon files exist)
            let markers = vec![
                MarkerOverlay {
                    coord: px_to_geo(100.0, 100.0, 200, 200),
                    icon_path: "__nonexistent_icon__".to_string(),
                    scale: 1.0,
                    offset: (0.0, 0.0),
                },
                MarkerOverlay {
                    coord: px_to_geo(60.0, 60.0, 200, 200),
                    icon_path: "__nonexistent_icon__".to_string(),
                    scale: 1.0,
                    offset: (0.0, 0.0),
                },
            ];
            draw_overlays(
                &mut img,
                &[],
                &markers,
                0.0,
                0.0,
                0.0,
                &OverlayDefaults::default(),
            );
            assert_image_snapshot("circle_markers", &img);
        }

        #[test]
        fn render_alpha_blending() {
            let mut img = blank_canvas(200, 200);
            let paths = vec![
                PathOverlay {
                    points: vec![
                        px_to_geo(20.0, 20.0, 200, 200),
                        px_to_geo(120.0, 20.0, 200, 200),
                        px_to_geo(120.0, 120.0, 200, 200),
                        px_to_geo(20.0, 120.0, 200, 200),
                    ],
                    stroke: None,
                    fill: parse_paint("rgba(255,0,0,0.5)"),
                    width: None,
                },
                PathOverlay {
                    points: vec![
                        px_to_geo(80.0, 80.0, 200, 200),
                        px_to_geo(180.0, 80.0, 200, 200),
                        px_to_geo(180.0, 180.0, 200, 200),
                        px_to_geo(80.0, 180.0, 200, 200),
                    ],
                    stroke: None,
                    fill: parse_paint("rgba(0,0,255,0.5)"),
                    width: None,
                },
            ];
            draw_test_paths(&mut img, &paths, &OverlayDefaults::default());
            assert_image_snapshot("alpha_blending", &img);
        }

        #[test]
        fn render_draw_overlays_full() {
            let mut img = blank_canvas(400, 300);
            let paths = vec![
                PathOverlay {
                    points: vec![
                        Coord { x: -1.0, y: 1.0 },
                        Coord { x: 0.0, y: -1.0 },
                        Coord { x: 1.0, y: 1.0 },
                    ],
                    stroke: parse_paint("rgb(0,0,200)"),
                    fill: parse_paint("rgba(100,100,255,0.314)"),
                    width: Some(3.0),
                },
                PathOverlay {
                    points: vec![
                        Coord { x: -1.5, y: 0.0 },
                        Coord { x: 1.5, y: 0.0 },
                    ],
                    stroke: parse_paint("rgb(200,0,0)"),
                    fill: None,
                    width: Some(5.0),
                },
            ];
            let defaults = OverlayDefaults {
                border: parse_paint("rgba(0,0,0,0.784)"),
                borderwidth: Some(2.0),
                stroke_style: {
                    let mut s = Stroke::default();
                    s.line_cap = tiny_skia::LineCap::Round;
                    s.line_join = tiny_skia::LineJoin::Round;
                    s
                },
                ..Default::default()
            };
            draw_overlays(&mut img, &paths, &[], 0.0, 0.0, 5.0, &defaults);
            assert_image_snapshot("overlays_full", &img);
        }

    }
}
