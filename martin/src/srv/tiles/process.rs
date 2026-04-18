use martin_core::tiles::Tile;
#[cfg(feature = "mlt")]
use martin_tile_utils::Format;

#[cfg(feature = "mlt")]
use crate::config::file::MltProcessConfig;
use crate::config::file::ProcessConfig;

/// Errors that can occur during tile post-processing.
#[derive(thiserror::Error, Debug)]
pub enum ProcessError {
    #[cfg(feature = "mlt")]
    #[error("MVT to MLT conversion failed: {0}")]
    MltConversion(String),
    #[cfg(feature = "mlt")]
    #[error("MLT encoding failed: {0}")]
    MltEncoding(String),
    #[error("Tile decompression failed: {0}")]
    DecompressionFailed(String),
}

impl From<ProcessError> for actix_web::Error {
    fn from(e: ProcessError) -> Self {
        actix_web::error::ErrorInternalServerError(e.to_string())
    }
}

/// Apply pre-cache postprocessors to a tile based on the resolved process config.
///
/// Currently supports:
/// - MLT conversion: converts MVT tiles to MLT format (requires `mlt` feature)
///
/// This runs before the tile is stored in cache, so cached tiles are already post-processed.
pub fn apply_pre_cache_processors(
    tile: Tile,
    config: &ProcessConfig,
) -> Result<Tile, ProcessError> {
    if tile.data.is_empty() {
        return Ok(tile);
    }

    // Step 1: Format conversion (MLT)
    #[cfg(feature = "mlt")]
    let tile = {
        let mut tile = tile;
        if let Some(mlt_config) = &config.mlt
            && tile.info.format == Format::Mvt
        {
            tile = convert_mvt_to_mlt(tile, mlt_config)?;
        }
        tile
    };
    #[cfg(not(feature = "mlt"))]
    let _ = &config;

    Ok(tile)
}

/// Convert an MVT tile to MLT format.
///
/// Handles decompression if the tile is compressed, then converts MVT→MLT
/// using `mlt-core`, and returns an uncompressed MLT tile.
#[cfg(feature = "mlt")]
fn convert_mvt_to_mlt(tile: Tile, mlt_config: &MltProcessConfig) -> Result<Tile, ProcessError> {
    use martin_tile_utils::{Encoding, TileInfo};

    let decoded = super::content::decode(tile)
        .map_err(|e| ProcessError::DecompressionFailed(e.to_string()))?;

    let tile_layers = mlt_core::mvt::mvt_to_tile_layers(decoded.data)
        .map_err(|e| ProcessError::MltConversion(e.to_string()))?;

    let cfg = mlt_config.to_encoder_config();
    let mut mlt_bytes = Vec::new();
    for layer in tile_layers {
        let layer_bytes = layer
            .encode(cfg)
            .map_err(|e| ProcessError::MltEncoding(e.to_string()))?;
        mlt_bytes.extend_from_slice(&layer_bytes);
    }

    Ok(Tile::new_hash_etag(
        mlt_bytes,
        TileInfo::new(Format::Mlt, Encoding::Uncompressed),
    ))
}

#[cfg(test)]
mod tests {
    use martin_core::tiles::Tile;
    use martin_tile_utils::{Encoding, Format, TileInfo};

    use super::*;

    fn make_tile(data: Vec<u8>, format: Format, encoding: Encoding) -> Tile {
        Tile::new_hash_etag(data, TileInfo::new(format, encoding))
    }

    /// Minimal valid MVT tile: one layer named "x", version=2, extent=4096, no features.
    #[cfg(feature = "mlt")]
    fn minimal_mvt() -> Vec<u8> {
        vec![0x1a, 0x08, 0x0a, 0x01, 0x78, 0x78, 0x02, 0x28, 0x80, 0x20]
    }

    #[cfg(feature = "mlt")]
    #[test]
    fn empty_tile_is_noop() {
        let tile = make_tile(Vec::new(), Format::Mvt, Encoding::Uncompressed);
        let config = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert!(result.data.is_empty());
    }

    #[test]
    fn no_mlt_config_is_noop() {
        let tile = make_tile(vec![1, 2, 3], Format::Mvt, Encoding::Uncompressed);
        let config = ProcessConfig::default();
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert_eq!(result.data, vec![1, 2, 3]);
        assert_eq!(result.info.format, Format::Mvt);
    }

    #[cfg(feature = "mlt")]
    #[test]
    fn non_mvt_format_is_noop() {
        let tile = make_tile(vec![1, 2, 3], Format::Png, Encoding::Internal);
        let config = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert_eq!(result.info.format, Format::Png);
    }

    #[cfg(feature = "mlt")]
    #[test]
    fn mvt_tile_converted_to_mlt() {
        let tile = make_tile(minimal_mvt(), Format::Mvt, Encoding::Uncompressed);
        let config = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert_eq!(result.info.format, Format::Mlt);
        assert_eq!(result.info.encoding, Encoding::Uncompressed);
    }

    #[cfg(feature = "mlt")]
    #[test]
    fn compressed_mvt_tile_decompressed_and_converted() {
        use martin_tile_utils::encode_gzip;

        let gzipped = encode_gzip(&minimal_mvt()).unwrap();
        let tile = make_tile(gzipped, Format::Mvt, Encoding::Gzip);
        let config = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert_eq!(result.info.format, Format::Mlt);
        assert_eq!(result.info.encoding, Encoding::Uncompressed);
    }
}
