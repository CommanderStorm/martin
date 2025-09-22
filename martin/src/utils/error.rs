use std::error::Error;
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

/// A convenience [`Result`] for Martin crate.
pub type MartinResult<T> = Result<T, MartinError>;

fn elide_vec(vec: &[String], max_items: usize, max_len: usize) -> String {
    let mut s = String::new();
    for (i, v) in vec.iter().enumerate() {
        if i > max_items {
            let _ = write!(s, " and {} more", vec.len() - i);
            break;
        }
        if i > 0 {
            s.push(' ');
        }
        if v.len() > max_len {
            s.push_str(&v[..max_len]);
            s.push('…');
        } else {
            s.push_str(v);
        }
    }
    s
}

#[derive(thiserror::Error, Debug)]
pub enum MartinError {
    #[error("The --config and the connection parameters cannot be used together. Please remove unsupported parameters '{}'", elide_vec(.0, 3, 15))]
    ConfigAndConnectionsError(Vec<String>),

    #[error("Unable to bind to {1}: {0}")]
    BindingError(io::Error, String),

    #[error("Unrecognizable connection strings: {0:?}")]
    UnrecognizableConnections(Vec<String>),

    #[cfg(any(
        feature = "postgres",
        feature = "pmtiles",
        feature = "mbtiles",
        feature = "cog"
    ))]
    #[error(transparent)]
    TileSourceError(#[from] martin_core::tiles::TileSourceError),

    #[error(transparent)]
    ConfigFileError(#[from] crate::config::file::ConfigFileError),

    #[cfg(feature = "sprites")]
    #[error(transparent)]
    SpriteError(#[from] martin_core::sprites::SpriteError),

    #[cfg(feature = "fonts")]
    #[error(transparent)]
    FontError(#[from] martin_core::fonts::FontError),

    #[error(transparent)]
    WebError(#[from] actix_web::Error),

    #[error(transparent)]
    IoError(#[from] io::Error),

    #[error("Internal error: {0}")]
    InternalError(#[from] Box<dyn Error + Send + Sync>),
}
