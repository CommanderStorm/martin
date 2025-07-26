use serde::{Deserialize, Serialize};
/// A global transition definition to use as a default across properties, to be used for timing transitions between one value and the next when no property-specific transition is set.
///
/// Collision-based symbol fading is controlled independently of the style's `transition` property.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct Transition {
    /// Optional number in range [0, ∞). Units in milliseconds. Defaults to `300`.
    ///
    /// Time allotted for transitions to complete.
    pub duration: Option<f32>,
    /// Optional number in range [0, ∞). Units in milliseconds. Defaults to `0`.
    ///
    /// Length of time before a transition begins.
    pub delay: Option<f32>,
}
