use serde::{Deserialize, Serialize};
/// The terrain configuration.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct Terrain {
    /// The source for the terrain data.
    ///
    /// Example: "raster-dem-source"
    pub source: String,
    /// Optional number in range [0, ∞). Defaults to `1`.
    ///
    /// The exaggeration of the terrain - how high it will look.
    pub exaggeration: f32,
}
