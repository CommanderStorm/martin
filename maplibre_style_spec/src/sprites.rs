use serde::{Deserialize, Serialize};
use url::Url;

/// # Sprites
///
/// Loading a [sprite](https://en.wikipedia.org/wiki/Sprite_(computer_graphics)) can be done using the optional `sprite` property at the root level of a MapLibre style sheet.
///
/// The images contained in the sprite can be referenced in other style properties (`background-pattern`, `fill-pattern`, `line-pattern`,`fill-extrusion-pattern` and `icon-image`).
///
/// ## Usage
///
/// You need to pass an URL where the sprite can be loaded from.
///
/// ```json
/// "sprite": "https://demotiles.maplibre.org/styles/osm-bright-gl-style/sprite"
/// ```
///
/// This will load both an image by appending `.png` and the metadata about the sprite needed for loading by appending `.json`.
/// See for yourself:
///
/// - [https://demotiles.maplibre.org/styles/osm-bright-gl-style/sprite.png](https://demotiles.maplibre.org/styles/osm-bright-gl-style/sprite.png)
/// - [https://demotiles.maplibre.org/styles/osm-bright-gl-style/sprite.json](https://demotiles.maplibre.org/styles/osm-bright-gl-style/sprite.json)
///
/// When a sprite is provided, you can refer to the images in the sprite in other parts of the style sheet.
/// For example, when creating a symbol layer with the layout property `"icon-image": "poi"`. Or with the tokenized value  `"icon-image": "{icon}"` and vector tile features with an `icon` property with the value `poi`.
///
/// Please consult the documentation for [information on the Sprite Source Format, HighDPI support and how to generate them](https://maplibre.org/maplibre-style-spec/sprite/).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
#[serde(untagged)]
pub enum Sprites {
    /// Single sprite
    ///
    /// URL where the sprite (`json`+`png`) are located.
    One(String),
    /// ### Multiple Sprite Sources
    ///
    /// You can also supply an array of `{ id: ..., url: ... }` pairs to load multiple sprites:
    ///
    /// As you can see, each sprite has an id.
    /// All images contained within a sprite also have an id.
    /// When using multiple sprites, you need to prefix the id of the image with the id of the sprite it is contained within, followed by a colon.
    /// For example, to reference the stop_sign image on the roadsigns sprite, you would need to use roadsigns:stop_sign.
    ///
    /// The sprite with id default is special in that you do not need to prefix the images contained within it.
    /// For example, to reference the image with id airport in the default sprite above, you can simply use airport.
    ///
    /// ```json
    /// [
    ///     {
    ///         "id": "roadsigns",
    ///         "url": "https://example.com/myroadsigns"
    ///     },
    ///     {
    ///         "id": "shops",
    ///         "url": "https://example2.com/someurl"
    ///     },
    ///     {
    ///         "id": "default",
    ///         "url": "https://example2.com/anotherurl"
    ///     }
    /// ]
    /// ```
    Many(Vec<Sprite>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct Sprite {
    /// The id of the sprite.
    pub id: String,
    /// URL where the sprite (`json`+`png`) are located.
    pub url: Url,
}
