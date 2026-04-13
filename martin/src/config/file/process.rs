use serde::{Deserialize, Serialize};
#[cfg(feature = "mlt")]
use mlt_core::encoder::EncoderConfig;

/// Postprocessing pipeline configuration.
///
/// Can appear at three levels: global, source-type, and per-source.
/// Merge strategy is full override: if a lower level specifies `process`,
/// it completely replaces the inherited config.
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessConfig {
    /// MVT-to-MLT format conversion.
    /// - `mlt: auto` — use default encoder settings (this is the default)
    /// - `mlt: { tessellate: true, ... }` — explicit encoder config overrides
    /// - absent — no MLT conversion
    pub mlt: Option<MltProcessConfig>,
    /// Ordered list of available compression encodings.
    /// First = most preferred. Only these are offered to clients.
    /// e.g. `["br", "gzip"]` or `["gzip", "plain"]`
    pub compress: Option<Vec<String>>,
}

/// Configuration for MVT-to-MLT format conversion.
///
/// - `"auto"` — use `mlt-core`'s default `EncoderConfig`
/// - An object with explicit fields — override specific encoder settings
///
/// Deserialized from either the string `"auto"` or a config object.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum MltProcessConfig {
    /// Use default encoder settings.
    #[default]
    Auto,
    /// Explicit encoder configuration overrides.
    Explicit(MltEncoderConfig),
}

impl Serialize for MltProcessConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Explicit(cfg) => cfg.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MltProcessConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;

        let value = serde_yaml::Value::deserialize(deserializer)?;
        match &value {
            serde_yaml::Value::String(s) if s == "auto" => Ok(Self::Auto),
            serde_yaml::Value::String(s) => {
                Err(D::Error::custom(format!("expected \"auto\", got \"{s}\"")))
            }
            serde_yaml::Value::Mapping(_) => {
                let cfg = MltEncoderConfig::deserialize(value).map_err(D::Error::custom)?;
                Ok(Self::Explicit(cfg))
            }
            _ => Err(D::Error::custom(
                "expected \"auto\" or an object with encoder settings",
            )),
        }
    }
}

/// Explicit encoder configuration for MLT conversion.
/// All fields are optional; unset fields use `mlt-core`'s defaults.
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MltEncoderConfig {
    /// Generate tessellation data for polygons and multi-polygons.
    pub tessellate: Option<bool>,
    /// Try sorting features by Z-order (Morton) curve index of their first vertex.
    pub try_spatial_morton_sort: Option<bool>,
    /// Try sorting features by Hilbert curve index of their first vertex.
    pub try_spatial_hilbert_sort: Option<bool>,
    /// Try sorting features by their feature ID in ascending order.
    pub try_id_sort: Option<bool>,
    /// Allow FSST string compression.
    pub allow_fsst: Option<bool>,
    /// Allow `FastPFOR` integer compression.
    pub allow_fpf: Option<bool>,
    /// Allow string grouping into shared dictionaries.
    pub allow_shared_dict: Option<bool>,
}

#[cfg(feature = "mlt")]
impl MltProcessConfig {
    /// Convert to `EncoderConfig`.
    #[must_use]
    pub fn to_encoder_config(&self) -> EncoderConfig {
        match self {
            Self::Auto => EncoderConfig::default(),
            Self::Explicit(cfg) => EncoderConfig::from(cfg.clone()),
        }
    }
}

/// Applying `MltEncoderConfig` overrides on top of `EncoderConfig` defaults.
///
/// Uses exhaustive destructuring of both structs so that adding a field
/// to either `MltEncoderConfig` or `EncoderConfig` causes a compile error
/// until this conversion is updated.
#[cfg(feature = "mlt")]
impl From<MltEncoderConfig> for EncoderConfig {
    fn from(src: MltEncoderConfig) -> Self {
        // Destructure both so new fields cause a compile error.
        let MltEncoderConfig {
            tessellate,
            try_spatial_morton_sort,
            try_spatial_hilbert_sort,
            try_id_sort,
            allow_fsst,
            allow_fpf,
            allow_shared_dict,
        } = src;

        let Self {
            tessellate: d_tessellate,
            try_spatial_morton_sort: d_morton,
            try_spatial_hilbert_sort: d_hilbert,
            try_id_sort: d_id,
            allow_fsst: d_fsst,
            allow_fpf: d_fpf,
            allow_shared_dict: d_shared,
        } = Self::default();

        Self {
            tessellate: tessellate.unwrap_or(d_tessellate),
            try_spatial_morton_sort: try_spatial_morton_sort.unwrap_or(d_morton),
            try_spatial_hilbert_sort: try_spatial_hilbert_sort.unwrap_or(d_hilbert),
            try_id_sort: try_id_sort.unwrap_or(d_id),
            allow_fsst: allow_fsst.unwrap_or(d_fsst),
            allow_fpf: allow_fpf.unwrap_or(d_fpf),
            allow_shared_dict: allow_shared_dict.unwrap_or(d_shared),
        }
    }
}

/// Resolve effective process config using full-override semantics:
/// per-source > source-type > global > default.
#[must_use]
pub fn resolve_process_config(
    global: Option<&ProcessConfig>,
    source_type: Option<&ProcessConfig>,
    per_source: Option<&ProcessConfig>,
) -> ProcessConfig {
    per_source
        .or(source_type)
        .or(global)
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn parse_empty() {
        let cfg: ProcessConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(cfg, ProcessConfig::default());
    }

    #[test]
    fn parse_mlt_auto_string() {
        let cfg: ProcessConfig = serde_yaml::from_str(indoc! {"
            mlt: auto
        "})
        .unwrap();
        assert_eq!(
            cfg,
            ProcessConfig {
                mlt: Some(MltProcessConfig::Auto),
                compress: None,
            }
        );
    }

    #[test]
    fn parse_mlt_explicit_empty() {
        let cfg: ProcessConfig = serde_yaml::from_str(indoc! {"
            mlt: {}
        "})
        .unwrap();
        assert_eq!(
            cfg,
            ProcessConfig {
                mlt: Some(MltProcessConfig::Explicit(MltEncoderConfig::default())),
                compress: None,
            }
        );
    }

    #[test]
    fn parse_mlt_explicit_with_overrides() {
        let cfg: ProcessConfig = serde_yaml::from_str(indoc! {"
            mlt:
              tessellate: true
              allow_fsst: false
        "})
        .unwrap();
        assert_eq!(
            cfg,
            ProcessConfig {
                mlt: Some(MltProcessConfig::Explicit(MltEncoderConfig {
                    tessellate: Some(true),
                    allow_fsst: Some(false),
                    ..Default::default()
                })),
                compress: None,
            }
        );
    }

    #[test]
    fn parse_compress_only() {
        let cfg: ProcessConfig = serde_yaml::from_str(indoc! {"
            compress:
              - br
              - gzip
        "})
        .unwrap();
        assert_eq!(
            cfg,
            ProcessConfig {
                mlt: None,
                compress: Some(vec!["br".to_string(), "gzip".to_string()]),
            }
        );
    }

    #[test]
    fn parse_full() {
        let cfg: ProcessConfig = serde_yaml::from_str(indoc! {"
            mlt: auto
            compress:
              - br
              - gzip
        "})
        .unwrap();
        assert_eq!(
            cfg,
            ProcessConfig {
                mlt: Some(MltProcessConfig::Auto),
                compress: Some(vec!["br".to_string(), "gzip".to_string()]),
            }
        );
    }

    #[test]
    fn serde_round_trip_auto() {
        let cfg = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
            compress: Some(vec!["zstd".to_string()]),
        };
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        assert!(yaml.contains("mlt: auto"));
        let parsed: ProcessConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn serde_round_trip_explicit() {
        let cfg = ProcessConfig {
            mlt: Some(MltProcessConfig::Explicit(MltEncoderConfig {
                tessellate: Some(true),
                ..Default::default()
            })),
            compress: Some(vec!["gzip".to_string()]),
        };
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let parsed: ProcessConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn parse_mlt_invalid_string() {
        let result = serde_yaml::from_str::<ProcessConfig>(indoc! {"
            mlt: invalid
        "});
        assert!(result.is_err());
    }

    #[test]
    fn parse_mlt_invalid_type() {
        let result = serde_yaml::from_str::<ProcessConfig>(indoc! {"
            mlt: 123
        "});
        assert!(result.is_err());
    }

    #[test]
    fn resolve_per_source_overrides_all() {
        let global = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
            compress: Some(vec!["gzip".to_string()]),
        };
        let source_type = ProcessConfig {
            mlt: None,
            compress: Some(vec!["br".to_string()]),
        };
        let per_source = ProcessConfig {
            mlt: None,
            compress: Some(vec!["zstd".to_string()]),
        };

        let resolved =
            resolve_process_config(Some(&global), Some(&source_type), Some(&per_source));
        assert_eq!(resolved, per_source);
    }

    #[test]
    fn resolve_source_type_overrides_global() {
        let global = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
            compress: Some(vec!["gzip".to_string()]),
        };
        let source_type = ProcessConfig {
            mlt: None,
            compress: Some(vec!["br".to_string()]),
        };

        let resolved = resolve_process_config(Some(&global), Some(&source_type), None);
        assert_eq!(resolved, source_type);
    }

    #[test]
    fn resolve_global_used_as_fallback() {
        let global = ProcessConfig {
            mlt: Some(MltProcessConfig::Auto),
            compress: Some(vec!["gzip".to_string()]),
        };

        let resolved = resolve_process_config(Some(&global), None, None);
        assert_eq!(resolved, global);
    }

    #[test]
    fn resolve_default_when_all_none() {
        let resolved = resolve_process_config(None, None, None);
        assert_eq!(resolved, ProcessConfig::default());
    }
}
