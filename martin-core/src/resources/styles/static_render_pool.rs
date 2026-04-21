use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{mpsc, LazyLock};
use std::thread;

use maplibre_native::{Height, Image, ImageRendererBuilder, RenderingError, Size, Width};
use tokio::sync::oneshot;

/// Request to render a static map image.
struct StaticRenderRequest {
    style_path: PathBuf,
    lat: f64,
    lon: f64,
    zoom: f64,
    bearing: f64,
    pitch: f64,
    width: u32,
    height: u32,
    pixel_ratio: f32,
    response: oneshot::Sender<Result<Image, StaticRenderPoolError>>,
}

/// A thread-safe rendering pool for static map images.
///
/// Serializes rendering operations through a single worker thread.
/// Uses `set_map_size()` for dimension changes (cheap) and rebuilds only
/// when `pixel_ratio` changes (rare).
#[derive(Debug, Clone)]
pub struct StaticRenderPool {
    rendering_requests: mpsc::Sender<StaticRenderRequest>,
}

impl StaticRenderPool {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel::<StaticRenderRequest>();

        thread::spawn(move || {
            let mut pixel_ratio: f32 = 1.0;
            let mut renderer = ImageRendererBuilder::default().build_static_renderer();
            let mut current_style: Option<PathBuf> = None;
            let mut current_width: u32 = 512;
            let mut current_height: u32 = 512;

            while let Ok(request) = rx.recv() {
                // Rebuild renderer if pixel_ratio changed
                if (request.pixel_ratio - pixel_ratio).abs() > 0.01 {
                    pixel_ratio = request.pixel_ratio;
                    renderer = ImageRendererBuilder::default()
                        .with_pixel_ratio(pixel_ratio)
                        .with_size(
                            NonZeroU32::new(request.width).unwrap_or(NonZeroU32::MIN),
                            NonZeroU32::new(request.height).unwrap_or(NonZeroU32::MIN),
                        )
                        .build_static_renderer();
                    current_width = request.width;
                    current_height = request.height;
                    // Force style reload after rebuild
                    current_style = None;
                }

                // Resize if dimensions changed (cheap FFI call)
                if request.width != current_width || request.height != current_height {
                    renderer.set_map_size(Size::new(
                        Width(request.width),
                        Height(request.height),
                    ));
                    current_width = request.width;
                    current_height = request.height;
                }

                // Load style if changed
                if current_style.as_ref() != Some(&request.style_path) {
                    if let Err(e) = renderer.load_style_from_path(&request.style_path) {
                        let _ = request
                            .response
                            .send(Err(StaticRenderPoolError::IOError(e)));
                        continue;
                    }
                    current_style = Some(request.style_path.clone());
                }

                let result = renderer
                    .render_static(
                        request.lat,
                        request.lon,
                        request.zoom,
                        request.bearing,
                        request.pitch,
                    )
                    .map_err(StaticRenderPoolError::RenderingError);
                let _ = request.response.send(result);
            }
        });

        Self {
            rendering_requests: tx,
        }
    }

    /// Render a static map image asynchronously.
    pub async fn render_static(
        &self,
        style_path: PathBuf,
        lat: f64,
        lon: f64,
        zoom: f64,
        bearing: f64,
        pitch: f64,
        width: u32,
        height: u32,
        pixel_ratio: f32,
    ) -> Result<Image, StaticRenderPoolError> {
        let (response_tx, response_rx) = oneshot::channel();

        self.rendering_requests
            .send(StaticRenderRequest {
                style_path,
                lat,
                lon,
                zoom,
                bearing,
                pitch,
                width,
                height,
                pixel_ratio,
                response: response_tx,
            })
            .map_err(|_| StaticRenderPoolError::FailedToSendRequest)?;

        response_rx
            .await
            .map_err(|_| StaticRenderPoolError::FailedToReceiveResponse)?
    }

    /// Get the global static rendering pool instance.
    #[must_use]
    pub fn global_pool() -> &'static Self {
        static GLOBAL_POOL: LazyLock<StaticRenderPool> = LazyLock::new(StaticRenderPool::new);
        &GLOBAL_POOL
    }
}

/// Errors that can occur in the static render pool.
#[derive(thiserror::Error, Debug)]
pub enum StaticRenderPoolError {
    /// An I/O error occurred during rendering operations.
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    /// A rendering error occurred during map rendering.
    #[error(transparent)]
    RenderingError(#[from] RenderingError),

    /// Failed to send a rendering request to the worker thread.
    #[error("Failed to send request to rendering thread")]
    FailedToSendRequest,

    /// Failed to receive a response from the worker thread.
    #[error("Failed to receive response from rendering thread")]
    FailedToReceiveResponse,
}
