use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde_with::skip_serializing_none]
pub struct Projection {
    /// The projection definition type.
    ///
    /// Can be specified as a string, a transition state, or an expression.
    ///
    /// Default value is "mercator".
    pub r#type: Option<Value>,
}
