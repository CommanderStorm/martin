use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct RootStyleSpec {
    /// Style specification version number.
    ///
    /// Must be `8`.
    pub version: u8,
    /// A human-readable name for the style.
    pub name: Option<String>,
    /// Arbitrary properties useful to track with the stylesheet, but do not influence rendering.
    ///
    /// Properties should be prefixed to avoid collisions, like `'maplibre:'`
    ///
    /// Example:
    ///
    /// ```json
    /// {
    ///     "styleeditor:slimmode": true,
    ///     "styleeditor:comment": "Style generated 1677776383",
    ///     "styleeditor:version": "3.14.159265",
    ///     "example:object": {
    ///         "String": "one",
    ///         "Number": 2,
    ///         "Boolean": false
    ///     }
    /// }
    /// ```
    pub metadata: Option<HashMap<String, Value>>,
    /// Default map center in longitude and latitude.
    ///
    /// The style center will be used only if the map has not been positioned by other means (e.g. map options or user interaction).
    ///
    /// Example: `[-73.9749, 40.7736]`
    pub center: Option<[f32; 2]>,
    /// Default map center altitude in meters above sea level.
    ///
    /// The style center altitude defines the altitude where the camera is looking at and will be used only if the map has not been positioned by other means (e.g. map options or user interaction).
    ///
    /// Example: `123.4`
    #[serde(rename = "centerAltitude")]
    pub center_altitude: Option<f32>,
    /// Default zoom level.
    ///
    /// The style zoom will be used only if the map has not been positioned by other means (e.g. map options or user interaction).
    ///
    /// Example: `12.5`
    pub zoom: Option<f32>,
    /// Default bearing, in degrees.
    ///
    /// The bearing is the compass direction that is "up"; for example, a bearing of 90° orients the map so that east is up.
    /// This value will be used only if the map has not been positioned by other means (e.g. map options or user interaction).
    ///
    /// Example: `29`
    pub bearing: Option<f32>,
    /// Default pitch, in degrees.
    ///
    /// Zero is perpendicular to the surface, for a look straight down at the map, while a greater value like 60 looks ahead towards the horizon.
    /// The style pitch will be used only if the map has not been positioned by other means (e.g. map options or user interaction).
    ///
    /// Example: `50`
    pub pitch: Option<f32>,
    /// Default roll, in degrees.
    ///
    /// The roll angle is measured counterclockwise about the camera boresight.
    /// The style roll will be used only if the map has not been positioned by other means (e.g. map options or user interaction).
    ///
    /// Example: `45`
    pub roll: Option<f32>,
    /// An object used to define default values when using the [`global-state`](https://maplibre.org/maplibre-style-spec/expressions/#global-state) expression.
    pub state: Option<HashMap<String, Value>>,
    /// The global light source.
    pub light: Option<super::Light>,
    /// The map's sky configuration.
    ///
    /// Note: this definition is still experimental and is under development in `maplibre-gl-js`.
    pub sky: Option<super::Sky>,
    /// The projection configuration
    ///
    /// # Example:
    ///
    /// ```json
    /// {
    ///   "type": [
    ///       "interpolate",
    ///       ["linear"],
    ///       ["zoom"],
    ///       10,
    ///       "vertical-perspective",
    ///       12,
    ///       "mercator"
    ///   ]
    /// }
    /// ```
    pub projection: Option<super::Projection>,
    /// The terrain configuration
    ///
    /// # Example:
    ///
    /// ```json
    /// {"source": "raster-dem-source", "exaggeration": 0.5}
    /// ```
    pub terrain: Option<super::Terrain>,
    /// Sources state which data the map should display.
    ///
    /// Specify the type of source with the `type` property.
    /// Adding a source isn't enough to make data appear on the map because sources don't contain styling details like color or width.
    /// Layers refer to a source and give it a visual representation.
    /// This makes it possible to style the same source in different ways, like differentiating between types of roads in a highways layer.
    ///
    /// Tiled sources (vector and raster) must specify their details according to the [TileJSON specification](https://github.com/mapbox/tilejson-spec).
    ///
    /// # Example:
    ///
    /// ```json
    /// {
    ///     "maplibre-demotiles": {
    ///         "type": "vector",
    ///         "url": "https://demotiles.maplibre.org/tiles/tiles.json"
    ///     },
    ///     "maplibre-tilejson": {
    ///         "type": "vector",
    ///         "url": "http://api.example.com/tilejson.json"
    ///     },
    ///     "maplibre-streets": {
    ///         "type": "vector",
    ///         "tiles": [
    ///             "http://a.example.com/tiles/{z}/{x}/{y}.pbf",
    ///             "http://b.example.com/tiles/{z}/{x}/{y}.pbf"
    ///         ],
    ///         "maxzoom": 14
    ///     },
    ///     "wms-imagery": {
    ///         "type": "raster",
    ///         "tiles": [
    ///             "http://a.example.com/wms?bbox={bbox-epsg-3857}&format=image/png&service=WMS&version=1.1.1&request=GetMap&srs=EPSG:3857&width=256&height=256&layers=example"
    ///         ],
    ///         "tileSize": 256
    ///     }
    /// }
    /// ```
    pub sources: Vec<super::Source>,
    /// An array of `{id: 'my-sprite', url: 'https://example.com/sprite'}` objects or a single string that represents a URL to load the sprite from.
    ///
    /// Each object should represent a unique URL to load a sprite from and and a unique ID to use as a prefix when referencing images from that sprite (i.e. 'my-sprite:image').
    /// All the URLs are internally extended to load both .json and .png files.
    /// If the id field is equal to 'default', the prefix is omitted (just 'image' instead of 'default:image').
    /// All the IDs and URLs must be unique.
    /// For backwards compatibility, instead of an array, one can also provide a single string that represents a URL to load the sprite from.
    /// The images in this case won't be prefixed.
    pub sprite: Option<super::Sprites>,
    /// A URL template for loading signed-distance-field glyph sets in PBF format.
    ///
    /// If this property is set, any text in the `text-field` layout property is displayed in the font stack named by the `text-font` layout property based on glyphs located at the URL specified by this property.
    /// Otherwise, font faces will be determined by the `text-font` property based on the local environment.
    ///
    /// The URL must include:
    ///
    /// - `{fontstack}` - When requesting glyphs, this token is replaced with a comma separated list of fonts from a font stack specified in the `text-font` property of a symbol layer.
    /// - `{range}` - When requesting glyphs, this token is replaced with a range of 256 Unicode code points.
    /// For example, to load glyphs for the Unicode Basic Latin and Basic Latin-1 Supplement blocks, the range would be 0-255.
    /// The actual ranges that are loaded are determined at runtime based on what text needs to be displayed.
    ///
    /// The URL must be absolute, containing the [scheme, authority and path components](https://en.wikipedia.org/wiki/URL#Syntax).
    ///
    /// Example: `https://demotiles.maplibre.org/font/{fontstack}/{range}.pbf`
    pub glyphs: Option<Url>,
    /// The `font-faces` property can be used to specify what font files to use for rendering text.
    ///
    /// Font faces contain information needed to render complex texts such as [Devanagari](https://en.wikipedia.org/wiki/Devanagari), [Khmer](https://en.wikipedia.org/wiki/Khmer_script) among many others.
    ///
    /// ## Unicode range
    ///
    /// The optional `unicode-range` property can be used to only use a particular font file for characters within the specified unicode range(s).
    /// Its value should be an array of strings, each indicating a start and end of a unicode range, similar to the [CSS descriptor with the same name](https://developer.mozilla.org/en-US/docs/Web/CSS/@font-face/unicode-range).
    /// This allows specifying multiple non-consecutive unicode ranges.
    /// When not specified, the default value is `U+0-10FFFF`, meaning the font file will be used for all unicode characters.
    ///
    /// Refer to the [Unicode Character Code Charts](https://www.unicode.org/charts/) to see ranges for scripts supported by Unicode.
    /// To see what unicode code-points are available in a font, use a tool like [FontDrop](https://fontdrop.info/).
    ///
    /// ## Font Resolution
    ///
    /// For every name in a symbol layer’s [`text-font`](./layers.md/#text-font) array, characters are matched if they are covered one of the by the font files in the corresponding entry of the `font-faces` map.
    /// Any still-unmatched characters then fall back to the [`glyphs`](./glyphs.md) URL if provided.
    ///
    /// ## Supported Fonts
    ///
    /// What type of fonts are supported is implementation-defined.
    /// Unsupported fonts are ignored.
    ///
    /// # Example:
    /// ```json
    /// {
    ///   "Noto Sans Regular": [
    ///       {
    ///           "url": "https://example.com/fonts/Noto%20Sans%20Regular/khmer.ttf",
    ///           "unicode-range": ["U+1780-17FF"]
    ///       },
    ///       {
    ///           "url": "https://example.com/fonts/Noto%20Sans%20Regular/myanmar.ttf",
    ///           "unicode-range": [
    ///               "U+1000-109F",
    ///               "U+A9E0-A9FF",
    ///               "U+AA60-AA7F"
    ///           ]
    ///       },
    ///       {
    ///           "url": "https://example.com/fonts/Noto%20Sans%20Regular/devanagari.ttf",
    ///           "unicode-range": ["U+0900-097F", "U+A8E0-A8FF"]
    ///       }
    ///   ],
    ///   "Open Sans": "https://example.com/fonts/Open%20Sans.ttf"
    /// }
    /// ```
    #[serde(rename = "font-faces")]
    pub font_faces: Option<HashMap<String, Value>>,
    /// A global transition definition to use as a default across properties, to be used for timing transitions between one value and the next when no property-specific transition is set.
    ///
    /// Collision-based symbol fading is controlled independently of the style's `transition` property.
    pub transition: Option<super::Transition>,
    /// A style's layers property lists all the layers available in that style.
    ///
    /// The type of layer is specified by the type property, and must be one of:
    /// - `background`,
    /// - `fill`,
    /// - `line`,
    /// - `symbol`,
    /// - `raster`,
    /// - `circle`,
    /// - `fill-extrusion`,
    /// - `heatmap`,
    /// - `hillshade`,
    /// - `color-relief`.
    ///
    /// Except for layers of the `background` type, each layer needs to refer to a source.
    /// Layers take the data that they get from a source, optionally filter features, and then define how those features are styled.
    pub layers: Vec<Value>,
}
