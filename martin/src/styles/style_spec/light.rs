use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct Light {
    /// Whether extruded geometries are lit relative to the map or viewport.
    pub anchor: Option<LightAnchor>,
    /// Position of the light source relative to lit (extruded) geometries.
    ///
    /// In [r radial coordinate, a azimuthal angle, p polar angle]:
    /// - r indicates the distance from the center of the base of an object to its light,
    /// - a indicates the position of the light (degrees proceed clockwise) relative to 0°:
    ///   - when `light.anchor` is set to `viewport`, 0° corresponds to the top of the viewport,
    ///   - when `light.anchor` is set to `map`, 0° corresponds to due north
    /// - p indicates the height of the light (from 0°, directly above, to 180°, directly below).
    pub position: Option<Value>,
    /// Color tint for lighting extruded geometries.
    ///
    /// Default: `#ffffff`
    pub color: Option<Value>,
    /// Intensity of lighting (on a scale from 0 to 1).
    ///
    /// Higher numbers will present as more extreme contrast.
    ///
    /// Default: `0.5`
    pub intensity: Option<Value>,
}

/// Whether extruded geometries are lit relative to the map or viewport.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub enum LightAnchor {
    /// The position of the light source is aligned to the rotation of the map.
    Map,
    /// The position of the light source is aligned to the rotation of the viewport
    #[default]
    Viewport,
}
