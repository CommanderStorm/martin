use maplibre_native::SingleThreadedRenderPoolError;
use crate::styles::static_render_pool::StaticRenderPoolError;

/// Errors that can occur during style processing.
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum StyleError {
    /// Cannot render style.
    #[error("Tile image cannot be rendered because {0}")]
    SingleThreadedRenderPoolError(#[from] SingleThreadedRenderPoolError),

    /// Cannot render static image.
    #[error("Static image cannot be rendered because {0}")]
    StaticRenderPoolError(#[from] StaticRenderPoolError),

    /// Rendering is disabled.
    #[error("Rendering is disabled")]
    RenderingIsDisabled,
}
