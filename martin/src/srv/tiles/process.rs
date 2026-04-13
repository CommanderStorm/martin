use actix_web::error::ErrorInternalServerError;
use actix_web::Result as ActixResult;
use martin_core::tiles::Tile;
use martin_tile_utils::Format;

use crate::config::file::{MltProcessConfig, ProcessConfig};

/// Apply pre-cache postprocessors to a tile based on the resolved process config.
///
/// Currently supports:
/// - MLT conversion: converts MVT tiles to MLT format (requires `mlt` feature)
///
/// This runs before the tile is stored in cache, so cached tiles are already post-processed.
pub fn apply_pre_cache_processors(tile: Tile, config: &ProcessConfig) -> ActixResult<Tile> {
    if tile.data.is_empty() {
        return Ok(tile);
    }

    let mut tile = tile;

    // Step 1: Format conversion (MLT)
    if let Some(mlt_config) = &config.mlt
        && tile.info.format == Format::Mvt
    {
        tile = convert_mvt_to_mlt(tile, mlt_config)?;
    }

    Ok(tile)
}

/// Convert an MVT tile to MLT format.
///
/// Handles decompression if the tile is compressed, then converts MVT→MLT
/// using `mlt-core`, and returns an uncompressed MLT tile.
#[cfg(feature = "mlt")]
fn convert_mvt_to_mlt(tile: Tile, mlt_config: &MltProcessConfig) -> ActixResult<Tile> {
    use martin_tile_utils::{Encoding, TileInfo};

    let decoded = super::content::decode(tile)?;

    let tile_layers = mlt_core::mvt::mvt_to_tile_layers(decoded.data)
        .map_err(|e| ErrorInternalServerError(format!("MVT to MLT conversion failed: {e}")))?;

    let cfg = mlt_config.to_encoder_config();
    let mut mlt_bytes = Vec::new();
    for layer in tile_layers {
        let layer_bytes = layer
            .encode(cfg)
            .map_err(|e| ErrorInternalServerError(format!("MLT encoding failed: {e}")))?;
        mlt_bytes.extend_from_slice(&layer_bytes);
    }

    Ok(Tile::new_hash_etag(
        mlt_bytes,
        TileInfo::new(Format::Mlt, Encoding::Uncompressed),
    ))
}

/// Stub when `mlt` feature is not enabled — returns an error.
#[cfg(not(feature = "mlt"))]
fn convert_mvt_to_mlt(_tile: Tile, _mlt_config: &MltProcessConfig) -> ActixResult<Tile> {
    Err(ErrorInternalServerError(
        "MLT conversion requested but the 'mlt' feature is not enabled. \
         Rebuild martin with --features mlt to enable MVT→MLT conversion.",
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
    fn minimal_mvt() -> Vec<u8> {
        vec![0x1a, 0x08, 0x0a, 0x01, 0x78, 0x78, 0x02, 0x28, 0x80, 0x20]
    }

    #[test]
    fn empty_tile_is_noop() {
        let tile = make_tile(Vec::new(), Format::Mvt, Encoding::Uncompressed);
        let config = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
            compress: None,
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert!(result.data.is_empty());
    }

    #[test]
    fn no_mlt_config_is_noop() {
        let tile = make_tile(vec![1, 2, 3], Format::Mvt, Encoding::Uncompressed);
        let config = ProcessConfig {
            mlt: None,
            compress: None,
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert_eq!(result.data, vec![1, 2, 3]);
        assert_eq!(result.info.format, Format::Mvt);
    }

    #[test]
    fn non_mvt_format_is_noop() {
        let tile = make_tile(vec![1, 2, 3], Format::Png, Encoding::Internal);
        let config = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
            compress: None,
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
            compress: None,
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
            compress: None,
        };
        let result = apply_pre_cache_processors(tile, &config).unwrap();
        assert_eq!(result.info.format, Format::Mlt);
        assert_eq!(result.info.encoding, Encoding::Uncompressed);
    }
}
