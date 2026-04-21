use actix_web::http::header::ContentType;
use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpResponse, route};
use martin_core::styles::StyleSources;
use serde::Deserialize;
use tracing::{error, trace, warn};

use super::static_overlay::{
    self, OverlayDefaults, bbox_to_center_zoom, calculate_bounds, parse_linecap, parse_linejoin,
    parse_marker, parse_paint, parse_path,
};

/// Path parameters for static image endpoints.
#[derive(Deserialize, Debug)]
struct StaticImagePath {
    style_id: String,
    /// "lon,lat,zoom[@bearing[,pitch]]" or "auto" or "minx,miny,maxx,maxy"
    static_params: String,
    /// "800x600.png" or "800x600@2x.jpeg"
    size_fmt: String,
}

/// Overlay parameters shared by query string (GET) and JSON body (POST).
#[derive(Deserialize, Debug, Default)]
struct OverlayParams {
    path: Option<Vec<String>>,
    marker: Option<Vec<String>>,
    latlng: Option<bool>,
    fill: Option<String>,
    stroke: Option<String>,
    width: Option<f32>,
    linecap: Option<String>,
    linejoin: Option<String>,
    border: Option<String>,
    borderwidth: Option<f32>,
    padding: Option<f64>,
    maxzoom: Option<f64>,
}

/// Parsed static map request type.
#[derive(Debug)]
enum StaticType {
    Center {
        lon: f64,
        lat: f64,
        zoom: f64,
        bearing: f64,
        pitch: f64,
    },
    BoundingBox {
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
    },
    Auto,
}

/// Parsed image size and format.
#[derive(Debug)]
struct SizeFormat {
    width: u32,
    height: u32,
    scale: f32,
    format: ImageFormat,
}

#[derive(Debug, Clone, Copy)]
enum ImageFormat {
    Png,
    Jpeg,
}

/// Configurable render limits.
const MAX_WIDTH: u32 = 2048;
const MAX_HEIGHT: u32 = 2048;
const MAX_SCALE: u8 = 4;

/// Parse static_params into a `StaticType`.
///
/// Formats:
/// - `lon,lat,zoom` — center
/// - `lon,lat,zoom@bearing` — center with bearing
/// - `lon,lat,zoom@bearing,pitch` — center with bearing and pitch
/// - `minx,miny,maxx,maxy` — bounding box
/// - `auto` — auto-fit from overlays
fn parse_static_params(s: &str) -> Option<StaticType> {
    if s == "auto" {
        return Some(StaticType::Auto);
    }

    // Check for `@` — indicates center mode with bearing[,pitch].
    // The `@` separates zoom from bearing, and bearing/pitch are comma-separated
    // after it, so we must split on `@` first to avoid confusing those commas
    // with the lon,lat,zoom commas.
    if let Some((before_at, after_at)) = s.split_once('@') {
        let mut parts = before_at.splitn(3, ',');
        let lon: f64 = parts.next()?.parse().ok()?;
        let lat: f64 = parts.next()?.parse().ok()?;
        let zoom: f64 = parts.next()?.parse().ok()?;

        let (bearing, pitch) = if let Some((b, p)) = after_at.split_once(',') {
            (b.parse::<f64>().ok()?, p.parse::<f64>().ok()?)
        } else {
            (after_at.parse::<f64>().ok()?, 0.0)
        };

        return Some(StaticType::Center {
            lon,
            lat,
            zoom,
            bearing,
            pitch,
        });
    }

    let parts: Vec<&str> = s.split(',').collect();
    match parts.len() {
        // lon,lat,zoom (center, no bearing/pitch)
        3 => {
            let lon: f64 = parts[0].parse().ok()?;
            let lat: f64 = parts[1].parse().ok()?;
            let zoom: f64 = parts[2].parse().ok()?;
            Some(StaticType::Center {
                lon,
                lat,
                zoom,
                bearing: 0.0,
                pitch: 0.0,
            })
        }
        // minx,miny,maxx,maxy (bounding box)
        4 => {
            let min_lon: f64 = parts[0].parse().ok()?;
            let min_lat: f64 = parts[1].parse().ok()?;
            let max_lon: f64 = parts[2].parse().ok()?;
            let max_lat: f64 = parts[3].parse().ok()?;
            Some(StaticType::BoundingBox {
                min_lon,
                min_lat,
                max_lon,
                max_lat,
            })
        }
        _ => None,
    }
}

/// Parse size_fmt into `SizeFormat`: `"800x600.png"`, `"800x600@2x.jpeg"`.
fn parse_size_format(s: &str) -> Option<SizeFormat> {
    let (size_part, ext) = s.rsplit_once('.')?;
    let format = match ext.to_ascii_lowercase().as_str() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        _ => return None,
    };

    let (dims, scale) = if let Some((dims, scale_str)) = size_part.split_once('@') {
        let scale_str = scale_str.strip_suffix('x').unwrap_or(scale_str);
        let scale: f32 = scale_str.parse().ok()?;
        (dims, scale)
    } else {
        (size_part, 1.0)
    };

    let (w_str, h_str) = dims.split_once('x')?;
    let width: u32 = w_str.parse().ok()?;
    let height: u32 = h_str.parse().ok()?;

    Some(SizeFormat {
        width,
        height,
        scale,
        format,
    })
}

impl OverlayParams {
    /// Merge another set of params into this one.
    /// `self` (query) takes precedence for scalars; vecs are concatenated.
    fn merge(&mut self, other: Self) {
        if let Some(mut other_paths) = other.path {
            self.path.get_or_insert_with(Vec::new).append(&mut other_paths);
        }
        if let Some(mut other_markers) = other.marker {
            self.marker.get_or_insert_with(Vec::new).append(&mut other_markers);
        }
        self.latlng = self.latlng.or(other.latlng);
        self.fill = self.fill.take().or(other.fill);
        self.stroke = self.stroke.take().or(other.stroke);
        self.width = self.width.or(other.width);
        self.linecap = self.linecap.take().or(other.linecap);
        self.linejoin = self.linejoin.take().or(other.linejoin);
        self.border = self.border.take().or(other.border);
        self.borderwidth = self.borderwidth.or(other.borderwidth);
        self.padding = self.padding.or(other.padding);
        self.maxzoom = self.maxzoom.or(other.maxzoom);
    }
}

/// Handle GET requests for static map images.
#[route(
    "/style/{style_id}/static/{static_params}/{size_fmt}",
    method = "GET"
)]
#[hotpath::measure]
pub async fn get_style_static(
    path: Path<StaticImagePath>,
    query: Query<OverlayParams>,
    styles: Data<StyleSources>,
) -> HttpResponse {
    handle_static_request(&path, query.into_inner(), &styles).await
}

/// Handle POST requests for static map images with JSON body.
#[route(
    "/style/{style_id}/static/{static_params}/{size_fmt}",
    method = "POST"
)]
#[hotpath::measure]
pub async fn post_style_static(
    path: Path<StaticImagePath>,
    query: Query<OverlayParams>,
    body: Json<OverlayParams>,
    styles: Data<StyleSources>,
) -> HttpResponse {
    let mut params = query.into_inner();
    params.merge(body.into_inner());
    handle_static_request(&path, params, &styles).await
}

async fn handle_static_request(
    path: &StaticImagePath,
    params: OverlayParams,
    styles: &StyleSources,
) -> HttpResponse {
    use martin_core::styles::StyleError;

    let style_id = &path.style_id;
    let Some(style_path) = styles.style_json_path(style_id) else {
        return HttpResponse::NotFound()
            .content_type(ContentType::plaintext())
            .body("No such style exists");
    };

    let Some(static_type) = parse_static_params(&path.static_params) else {
        return HttpResponse::BadRequest()
            .content_type(ContentType::plaintext())
            .body("Invalid static parameters. Expected: lon,lat,zoom or minx,miny,maxx,maxy or auto");
    };

    let Some(size_fmt) = parse_size_format(&path.size_fmt) else {
        return HttpResponse::BadRequest()
            .content_type(ContentType::plaintext())
            .body("Invalid size/format. Expected: {width}x{height}[@{scale}x].{png|jpeg}");
    };

    let pixel_width = size_fmt.width;
    let pixel_height = size_fmt.height;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let scale_u8 = size_fmt.scale.round() as u8;

    if pixel_width == 0 || pixel_height == 0 {
        return HttpResponse::BadRequest()
            .content_type(ContentType::plaintext())
            .body("Image dimensions must be greater than zero");
    }
    if pixel_width > MAX_WIDTH || pixel_height > MAX_HEIGHT {
        return HttpResponse::BadRequest()
            .content_type(ContentType::plaintext())
            .body(format!(
                "Image dimensions exceed maximum allowed ({MAX_WIDTH}x{MAX_HEIGHT})"
            ));
    }
    if scale_u8 > MAX_SCALE {
        return HttpResponse::BadRequest()
            .content_type(ContentType::plaintext())
            .body(format!(
                "Scale factor exceeds maximum allowed ({MAX_SCALE})"
            ));
    }

    let latlng = params.latlng.unwrap_or(false);

    let paths: Vec<_> = params
        .path
        .as_ref()
        .map(|ps| ps.iter().filter_map(|s| parse_path(s, latlng)).collect())
        .unwrap_or_default();
    let markers: Vec<_> = params
        .marker
        .as_ref()
        .map(|ms| ms.iter().filter_map(|s| parse_marker(s, latlng)).collect())
        .unwrap_or_default();

    let (center_lon, center_lat, zoom, bearing, pitch) = match static_type {
        StaticType::Center {
            lon,
            lat,
            zoom,
            bearing,
            pitch,
        } => (lon, lat, zoom, bearing, pitch),
        StaticType::BoundingBox {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
        } => {
            let padding = params.padding.unwrap_or(0.0);
            let (clon, clat, z) = bbox_to_center_zoom(
                min_lon,
                min_lat,
                max_lon,
                max_lat,
                pixel_width,
                pixel_height,
                padding,
                params.maxzoom,
            );
            (clon, clat, z, 0.0, 0.0)
        }
        StaticType::Auto => {
            let Some(bounds) = calculate_bounds(&paths, &markers) else {
                return HttpResponse::BadRequest()
                    .content_type(ContentType::plaintext())
                    .body(
                        "Auto mode requires at least one path or marker overlay",
                    );
            };
            let padding = params.padding.unwrap_or(0.1);
            let (clon, clat, z) = bbox_to_center_zoom(
                bounds.0,
                bounds.1,
                bounds.2,
                bounds.3,
                pixel_width,
                pixel_height,
                padding,
                params.maxzoom,
            );
            (clon, clat, z, 0.0, 0.0)
        }
    };

    #[allow(clippy::cast_precision_loss)]
    let render_width = ((pixel_width as f64) * f64::from(size_fmt.scale)).round();
    #[allow(clippy::cast_precision_loss)]
    let render_height = ((pixel_height as f64) * f64::from(size_fmt.scale)).round();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let render_width = render_width as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let render_height = render_height as u32;

    trace!(
        "Rendering static image for style {style_id} at ({center_lon},{center_lat}) z{zoom} {render_width}x{render_height}@{scale}",
        scale = size_fmt.scale,
    );

    let image = styles
        .render_static(
            style_path,
            center_lat,
            center_lon,
            zoom,
            bearing,
            pitch,
            render_width,
            render_height,
            size_fmt.scale,
        )
        .await;

    let image = match image {
        Ok(image) => image,
        Err(StyleError::RenderingIsDisabled) => {
            warn!(
                "Failed to render static image for style {style_id} because rendering is disabled"
            );
            return HttpResponse::Forbidden()
                .content_type(ContentType::plaintext())
                .body("Rendering is disabled");
        }
        Err(e) => {
            error!("Failed to render static image for style {style_id}: {e}");
            return HttpResponse::InternalServerError()
                .content_type(ContentType::plaintext())
                .body("Failed to render static image");
        }
    };

    let has_overlays = !paths.is_empty() || !markers.is_empty();
    let img_to_encode = if has_overlays {
        let mut img_buffer = image.as_image().clone();
        let mut stroke_style = tiny_skia::Stroke::default();
        stroke_style.line_cap = params
            .linecap
            .as_deref()
            .map(parse_linecap)
            .unwrap_or(tiny_skia::LineCap::Round);
        stroke_style.line_join = params
            .linejoin
            .as_deref()
            .map(parse_linejoin)
            .unwrap_or(tiny_skia::LineJoin::Round);

        let defaults = OverlayDefaults {
            fill: params.fill.as_deref().and_then(parse_paint),
            stroke: params.stroke.as_deref().and_then(parse_paint),
            width: params.width,
            stroke_style,
            border: params.border.as_deref().and_then(parse_paint),
            borderwidth: params.borderwidth,
        };

        static_overlay::draw_overlays(
            &mut img_buffer,
            &paths,
            &markers,
            center_lon,
            center_lat,
            zoom,
            &defaults,
        );
        std::borrow::Cow::Owned(img_buffer)
    } else {
        std::borrow::Cow::Borrowed(image.as_image())
    };

    let mut output = std::io::Cursor::new(Vec::new());
    let (image_format, content_type) = match size_fmt.format {
        ImageFormat::Png => (image::ImageFormat::Png, ContentType::png()),
        ImageFormat::Jpeg => (image::ImageFormat::Jpeg, ContentType::jpeg()),
    };

    match img_to_encode.write_to(&mut output, image_format) {
        Ok(()) => HttpResponse::Ok()
            .content_type(content_type)
            .body(output.into_inner()),
        Err(e) => {
            error!("Failed to encode static image: {e}");
            HttpResponse::InternalServerError()
                .content_type(ContentType::plaintext())
                .body("Failed to encode image")
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn parse_static_params_center() {
        let st = parse_static_params("-122.4,37.8,12").unwrap();
        match st {
            StaticType::Center {
                lon,
                lat,
                zoom,
                bearing,
                pitch,
            } => {
                assert!((lon - (-122.4)).abs() < 1e-10);
                assert!((lat - 37.8).abs() < 1e-10);
                assert!((zoom - 12.0).abs() < 1e-10);
                assert!((bearing - 0.0).abs() < 1e-10);
                assert!((pitch - 0.0).abs() < 1e-10);
            }
            _ => panic!("Expected Center"),
        }
    }

    #[test]
    fn parse_static_params_center_with_bearing() {
        let st = parse_static_params("-122.4,37.8,12@45").unwrap();
        match st {
            StaticType::Center {
                bearing, pitch, ..
            } => {
                assert!((bearing - 45.0).abs() < 1e-10);
                assert!((pitch - 0.0).abs() < 1e-10);
            }
            _ => panic!("Expected Center"),
        }
    }

    #[test]
    fn parse_static_params_center_with_bearing_pitch() {
        let st = parse_static_params("-122.4,37.8,12@45,60").unwrap();
        match st {
            StaticType::Center {
                bearing, pitch, ..
            } => {
                assert!((bearing - 45.0).abs() < 1e-10);
                assert!((pitch - 60.0).abs() < 1e-10);
            }
            _ => panic!("Expected Center"),
        }
    }

    #[test]
    fn parse_static_params_fractional_zoom() {
        let st = parse_static_params("10.5,20.3,5.5").unwrap();
        match st {
            StaticType::Center {
                lon, lat, zoom, ..
            } => {
                assert!((lon - 10.5).abs() < 1e-10);
                assert!((lat - 20.3).abs() < 1e-10);
                assert!((zoom - 5.5).abs() < 1e-10);
            }
            _ => panic!("Expected Center"),
        }
    }

    #[test]
    fn parse_static_params_bbox() {
        let st = parse_static_params("-123,37,-122,38").unwrap();
        match st {
            StaticType::BoundingBox {
                min_lon,
                min_lat,
                max_lon,
                max_lat,
            } => {
                assert!((min_lon - (-123.0)).abs() < 1e-10);
                assert!((min_lat - 37.0).abs() < 1e-10);
                assert!((max_lon - (-122.0)).abs() < 1e-10);
                assert!((max_lat - 38.0).abs() < 1e-10);
            }
            _ => panic!("Expected BoundingBox"),
        }
    }

    #[test]
    fn parse_static_params_negative_bbox() {
        let st = parse_static_params("-180,-90,180,90").unwrap();
        match st {
            StaticType::BoundingBox {
                min_lon,
                min_lat,
                max_lon,
                max_lat,
            } => {
                assert!((min_lon - (-180.0)).abs() < 1e-10);
                assert!((min_lat - (-90.0)).abs() < 1e-10);
                assert!((max_lon - 180.0).abs() < 1e-10);
                assert!((max_lat - 90.0).abs() < 1e-10);
            }
            _ => panic!("Expected BoundingBox"),
        }
    }

    #[test]
    fn parse_static_params_auto() {
        match parse_static_params("auto").unwrap() {
            StaticType::Auto => {}
            _ => panic!("Expected Auto"),
        }
    }

    #[rstest]
    #[case::word("invalid")]
    #[case::two_parts("1,2")]
    #[case::five_parts("1,2,3,4,5")]
    fn parse_static_params_invalid(#[case] input: &str) {
        assert!(parse_static_params(input).is_none());
    }

    #[test]
    fn parse_size_format_basic() {
        let sf = parse_size_format("800x600.png").unwrap();
        assert_eq!(sf.width, 800);
        assert_eq!(sf.height, 600);
        assert!((sf.scale - 1.0).abs() < f32::EPSILON);
        assert!(matches!(sf.format, ImageFormat::Png));
    }

    #[test]
    fn parse_size_format_with_scale() {
        let sf = parse_size_format("800x600@2x.jpeg").unwrap();
        assert_eq!(sf.width, 800);
        assert_eq!(sf.height, 600);
        assert!((sf.scale - 2.0).abs() < f32::EPSILON);
        assert!(matches!(sf.format, ImageFormat::Jpeg));
    }

    #[test]
    fn parse_size_format_jpg_alias() {
        let sf = parse_size_format("256x256.jpg").unwrap();
        assert!(matches!(sf.format, ImageFormat::Jpeg));
    }

    #[test]
    fn parse_size_format_scale_without_x_suffix() {
        let sf = parse_size_format("512x512@3.png").unwrap();
        assert!((sf.scale - 3.0).abs() < f32::EPSILON);
    }

    #[rstest]
    #[case::no_extension("not_valid")]
    #[case::unsupported_format("800x600.bmp")]
    #[case::missing_dot("800x600")]
    fn parse_size_format_invalid(#[case] input: &str) {
        assert!(parse_size_format(input).is_none());
    }

    #[test]
    fn merge_combines_paths() {
        let mut query = OverlayParams {
            path: Some(vec!["1,2|3,4".to_string()]),
            ..Default::default()
        };
        let body = OverlayParams {
            path: Some(vec!["5,6|7,8".to_string()]),
            ..Default::default()
        };
        query.merge(body);
        let paths = query.path.unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "1,2|3,4");
        assert_eq!(paths[1], "5,6|7,8");
    }

    #[test]
    fn merge_query_takes_precedence_for_scalars() {
        let mut query = OverlayParams {
            fill: Some("red".to_string()),
            width: Some(5.0),
            padding: Some(0.2),
            maxzoom: Some(15.0),
            ..Default::default()
        };
        let body = OverlayParams {
            fill: Some("blue".to_string()),
            width: Some(10.0),
            stroke: Some("green".to_string()),
            padding: Some(0.5),
            maxzoom: Some(18.0),
            ..Default::default()
        };
        query.merge(body);
        assert_eq!(query.fill.as_deref(), Some("red"));
        assert_eq!(query.width, Some(5.0));
        assert_eq!(query.stroke.as_deref(), Some("green"));
        assert_eq!(query.padding, Some(0.2));
        assert_eq!(query.maxzoom, Some(15.0));
    }

    #[test]
    fn merge_body_fills_gaps() {
        let mut query = OverlayParams::default();
        let body = OverlayParams {
            latlng: Some(true),
            linecap: Some("butt".to_string()),
            linejoin: Some("bevel".to_string()),
            border: Some("#ff0000".to_string()),
            borderwidth: Some(2.0),
            ..Default::default()
        };
        query.merge(body);
        assert_eq!(query.latlng, Some(true));
        assert_eq!(query.linecap.as_deref(), Some("butt"));
        assert_eq!(query.linejoin.as_deref(), Some("bevel"));
        assert_eq!(query.border.as_deref(), Some("#ff0000"));
        assert_eq!(query.borderwidth, Some(2.0));
    }

    #[test]
    fn merge_both_empty() {
        let mut query = OverlayParams::default();
        query.merge(OverlayParams::default());
        assert!(query.path.is_none());
        assert!(query.marker.is_none());
        assert!(query.fill.is_none());
        assert!(query.stroke.is_none());
        assert!(query.width.is_none());
    }

    #[test]
    fn merge_combines_markers() {
        let mut query = OverlayParams {
            marker: Some(vec!["1,2|icon1.png".to_string()]),
            ..Default::default()
        };
        let body = OverlayParams {
            marker: Some(vec!["3,4|icon2.png".to_string()]),
            ..Default::default()
        };
        query.merge(body);
        let markers = query.marker.unwrap();
        assert_eq!(markers.len(), 2);
    }
}
