use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The map's sky configuration.
///
/// # Note:
/// This definition is still experimental and is under development in `maplibre-gl-js`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct Sky {
    /// The base color for the sky.
    ///
    /// Default: `#88C6FC`
    #[serde(rename = "sky-color")]
    pub sky_color: Value,
    /// The base color at the horizon.
    ///
    /// Default: `#ffffff`
    #[serde(rename = "horizon-color")]
    pub horizon_color: Value,
    /// The base color for the fog.
    ///
    /// Requires 3D terrain.
    ///
    /// Default: `#ffffff`
    #[serde(rename = "fog-color")]
    pub fog_color: Value,
    /// How to blend the fog over the 3D terrain:
    ///
    /// - 0 is the map center and
    /// - 1 is the horizon.
    ///
    /// Requires 3D terrain.
    ///
    /// Default: `0.5`, Range: [0..1]
    #[serde(rename = "fog-ground-blend")]
    pub fog_ground_blend: Value,
    /// How to blend the fog color and the horizon color:
    ///
    /// - 0 is using the horizon color only and
    /// - 1 is using the fog color only
    ///
    /// Default: `0.8`, Range: [0..1]
    #[serde(rename = "horizon-fog-blend")]
    pub horizon_fog_blend: Value,
    /// How to blend the sky color and the horizon color:
    ///
    /// - 0 is not blending at all and using the sky color only and
    /// - 1 is blending the color at the middle of the sky.
    ///
    /// Default: `0.8`, Range: [0..1]
    #[serde(rename = "sky-horizon-blend")]
    pub sky_horizon_blend: Value,
    /// How to blend the atmosphere:
    ///
    /// - 0 is hidden and
    /// - 1 visible atmosphere.
    ///
    /// It is best to interpolate this expression when using globe projection.
    ///
    /// Default: `0.8`, Range: [0..1]
    #[serde(rename = "atmosphere-blend")]
    pub atmosphere_blend: Value,
}
