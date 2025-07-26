use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Sources state which data the map should display. Specify the type of source with the `type` property. Adding a source isn't enough to make data appear on the map because sources don't contain styling details like color or width. Layers refer to a source and give it a visual representation. This makes it possible to style the same source in different ways, like differentiating between types of roads in a highways layer.
///
/// Tiled sources (vector and raster) must specify their details according to the [TileJSON specification](https://github.com/mapbox/tilejson-spec).
///
/// # Example:
///
/// ```json
/// {
///   "maplibre-demotiles": {
///     "type": "vector",
///     "url": "https://demotiles.maplibre.org/tiles/tiles.json"
///   },
///   "maplibre-tilejson": {
///     "type": "vector",
///     "url": "http://api.example.com/tilejson.json"
///   },
///   "maplibre-streets": {
///     "type": "vector",
///     "tiles": [
///       "http://a.example.com/tiles/{z}/{x}/{y}.pbf",
///       "http://b.example.com/tiles/{z}/{x}/{y}.pbf"
///     ],
///     "maxzoom": 14
///   },
///   "wms-imagery": {
///     "type": "raster",
///     "tiles": [
///       "http://a.example.com/wms?bbox={bbox-epsg-3857}&format=image/png&service=WMS&version=1.1.1&request=GetMap&srs=EPSG:3857&width=256&height=256&layers=example"
///     ],
///     "tileSize": 256
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Source {
    /// A vector tile source.
    ///
    /// Tiles must be in [Mapbox Vector Tile format](https://github.com/mapbox/vector-tile-spec).
    /// All geometric coordinates in vector tiles must be between `-1 * extent` and `(extent * 2) - 1` inclusive.
    /// All layers that use a vector source must specify a [`source-layer`](https://maplibre.org/maplibre-style-spec/layers/#source-layer) value.
    Vector(VectorSource),
    /// A raster tile source.
    Raster(RasterSource),
    /// A raster DEM source.
    ///
    /// Only supports [Mapbox Terrain RGB](https://blog.mapbox.com/global-elevation-data-6689f1d0ba65) and Mapzen Terrarium tiles.
    RasterDem(RasterDemSource),
    /// A [GeoJSON](http://geojson.org/) source.
    ///
    /// Data must be provided via a `"data"` property, whose value can be a URL or inline GeoJSON. When using in a browser, the GeoJSON data must be on the same domain as the map or served with [CORS](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS) headers.
    GeoJson(GeoJsonSource),
    /// A video source
    ///
    /// When rendered as a [raster layer](https://maplibre.org/maplibre-style-spec/layers/#raster), the layer's [`raster-fade-duration`](https://maplibre.org/maplibre-style-spec/layers/#raster-fade-duration) property will cause the video to fade in.
    /// This happens when playback is started, paused and resumed, or when the video's coordinates are updated.
    /// To avoid this behavior, set the layer's [`raster-fade-duration`](https://maplibre.org/maplibre-style-spec/layers/#raster-fade-duration) property to `0`.
    Video(VideoSource),
    /// An image source
    Image(ImageSource),
}

/// Influences the y direction of the tile coordinates.
///
/// The global-mercator (aka Spherical Mercator) profile is assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum VectorScheme {
    /// Slippy map tilenames scheme
    XYZ,
    /// OSGeo spec scheme
    TMS,
}

/// A vector tile source.
///
/// Tiles must be in [Mapbox Vector Tile format](https://github.com/mapbox/vector-tile-spec).
/// All geometric coordinates in vector tiles must be between `-1 * extent` and `(extent * 2) - 1` inclusive.
/// All layers that use a vector source must specify a [`source-layer`](https://maplibre.org/maplibre-style-spec/layers/#source-layer) value.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct VectorSource {
    pub url: Option<String>,
    pub tiles: Option<Vec<String>>,
    pub bounds: Option<[f32; 4]>,
    /// Influences the y direction of the tile coordinates.
    ///
    /// The global-mercator (aka Spherical Mercator) profile is assumed.
    ///
    /// Default: [`VectorScheme::XYZ`]
    pub scheme: Option<VectorScheme>,
    /// Minimum zoom level for which tiles are available, as in the TileJSON spec.
    ///
    /// Default: `0`
    pub minzoom: Option<f32>,
    /// Maximum zoom level for which tiles are available, as in the TileJSON spec.
    /// Data from tiles at the maxzoom are used when displaying the map at higher zoom levels.
    ///
    /// Default: `22`
    pub maxzoom: Option<f32>,
    /// Contains an attribution to be displayed when the map is shown to a user.
    pub attribution: Option<String>,
    #[serde(rename = "promoteId")]
    pub promote_id: Option<String>,
    pub volatile: Option<bool>,
}

/// A raster tile source.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct RasterSource {
    pub url: Option<String>,
    pub tiles: Option<Vec<String>>,
    pub bounds: Option<[f32; 4]>,
    /// Minimum zoom level for which tiles are available, as in the TileJSON spec.
    ///
    /// Default: `0`
    pub minzoom: Option<f32>,

    /// Maximum zoom level for which tiles are available, as in the TileJSON spec.
    /// Data from tiles at the maxzoom are used when displaying the map at higher zoom levels.
    ///
    /// Default: `22`
    pub maxzoom: Option<f32>,
    #[serde(rename = "tileSize")]
    pub tile_size: Option<f32>,
    /// Influences the y direction of the tile coordinates.
    ///
    /// The global-mercator (aka Spherical Mercator) profile is assumed.
    ///
    /// Default: [`VectorScheme::XYZ`]
    pub scheme: Option<VectorScheme>,
    /// Contains an attribution to be displayed when the map is shown to a user.
    pub attribution: Option<String>,
    pub volatile: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct RasterDemSource {
    pub url: Option<String>,
    pub tiles: Option<Vec<String>>,
    pub bounds: Option<[f32; 4]>,
    /// Minimum zoom level for which tiles are available, as in the TileJSON spec.
    ///
    /// Default: `0`
    pub minzoom: Option<f32>,
    /// Maximum zoom level for which tiles are available, as in the TileJSON spec.
    /// Data from tiles at the maxzoom are used when displaying the map at higher zoom levels.
    ///
    /// Default: `22`
    pub maxzoom: Option<f32>,
    #[serde(rename = "tileSize")]
    pub tile_size: Option<f32>,
    /// Contains an attribution to be displayed when the map is shown to a user.
    pub attribution: Option<String>,
    pub encoding: Option<RasterEncoding>,
    pub volatile: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub enum RasterEncoding {
    Terrarium,
    #[default]
    Mapbox,
    Custom(CustomRasterEncoding),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct CustomRasterEncoding {
    #[serde(rename = "redFactor")]
    pub red_factor: Option<f32>,
    #[serde(rename = "blueFactor")]
    pub blue_factor: Option<f32>,
    #[serde(rename = "greenFactor")]
    pub green_factor: Option<f32>,
    #[serde(rename = "baseShift")]
    pub base_shift: Option<f32>,
}

/// A [GeoJSON](http://geojson.org/) source.
///
/// Data must be provided via a `"data"` property, whose value can be a URL or inline GeoJSON. When using in a browser, the GeoJSON data must be on the same domain as the map or served with [CORS](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS) headers.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct GeoJsonSource {
    pub data: String,
    /// Maximum zoom level for which tiles are available, as in the TileJSON spec.
    /// Data from tiles at the maxzoom are used when displaying the map at higher zoom levels.
    ///
    /// Default: `22`
    pub maxzoom: Option<f32>,
    /// Contains an attribution to be displayed when the map is shown to a user.
    pub attribution: Option<String>,
    pub buffer: Option<f32>,
    pub filter: Option<Value>,
    pub tolerance: Option<f32>,
    pub cluster: Option<f32>,
    #[serde(rename = "clusterRadius")]
    pub cluster_radius: Option<f32>,
    #[serde(rename = "clusterMaxZoom")]
    pub cluster_max_zoom: Option<f32>,
    #[serde(rename = "clusterMinPoints")]
    pub cluster_min_points: Option<f32>,
    #[serde(rename = "clusterProperties")]
    pub cluster_properties: Option<Value>,
    #[serde(rename = "lineMetrics")]
    pub line_metrics: Option<bool>,
    #[serde(rename = "generateId")]
    pub generate_id: Option<bool>,
    #[serde(rename = "promoteId")]
    pub promote_id: Option<String>,
}

/// [longitude, latitude] pair
type Coordinate = [f32; 2];

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct VideoSource {
    /// URLs to video content in order of preferred format.
    pub urls: Vec<String>,
    /// [`Coordinate`] pairs for the image corners listed in clockwise order:
    /// - top left,
    /// - top right,
    /// - bottom right,
    /// - bottom left.
    pub coordinates: Vec<Coordinate>,
}

/// An image source
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct ImageSource {
    /// URL that points to an image.
    pub url: String,
    /// [`Coordinate`] pairs for the image corners listed in clockwise order:
    /// - top left,
    /// - top right,
    /// - bottom right,
    /// - bottom left.
    pub coordinates: Vec<[f32; 2]>,
}
