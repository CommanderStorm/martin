//! The `normalized` `MBTiles` schema: a `map(z,x,y,tile_id)` table joined to an
//! `images(tile_id, tile_data)` table, deduplicating identical tiles. An optional
//! `tiles_with_hash` view exposes the `tile_id` as a hash, and an alternative
//! `tiles_shallow` + `tiles_data` variant uses an integer `tile_data_id`.

use sqlx::{SqliteExecutor, query};
use tracing::debug;

use crate::HashAlgorithm;
use crate::errors::MbtResult;
use crate::queries::create_schema;

pub async fn is_normalized_tables_type<T>(conn: &mut T) -> MbtResult<bool>
where
    for<'e> &'e mut T: SqliteExecutor<'e>,
{
    let sql = query!(
        "SELECT (
             -- Has 'map' and 'images' tables
             SELECT COUNT(*) = 2
             FROM sqlite_master
             WHERE (name = 'map' OR name = 'images')
                 AND type = 'table'
             --
         ) AND (
             -- 'map' table's columns and their types are as expected:
             -- 4 columns (zoom_level, tile_column, tile_row, tile_id).
             -- The order is not important
             SELECT COUNT(*) = 4
             FROM pragma_table_info('map')
             WHERE ((name = 'zoom_level' AND type LIKE '%INT%')
                 OR (name = 'tile_column' AND type LIKE '%INT%')
                 OR (name = 'tile_row' AND type LIKE '%INT%')
                 OR (name = 'tile_id' AND type = 'TEXT'))
             --
         ) AND (
             -- 'images' table's columns and their types are as expected:
             -- 2 columns (tile_id, tile_data).
             -- The order is not important
             SELECT COUNT(*) = 2
             FROM pragma_table_info('images')
             WHERE ((name = 'tile_id' AND type = 'TEXT')
                 OR (name = 'tile_data' AND type = 'BLOB'))
             --
         ) AS is_valid;"
    );

    Ok(sql.fetch_one(&mut *conn).await?.is_valid == 1)
}

/// Check if `MBTiles` has an alternative normalized schema with `tiles_shallow` + `tiles_data`
/// tables using integer `tile_data_id` instead of text `tile_id`.
pub async fn is_dedup_id_normalized_tables_type<T>(conn: &mut T) -> MbtResult<bool>
where
    for<'e> &'e mut T: SqliteExecutor<'e>,
{
    let sql = query!(
        "SELECT (
             -- Has 'tiles_shallow' and 'tiles_data' tables
             SELECT COUNT(*) = 2
             FROM sqlite_master
             WHERE (name = 'tiles_shallow' OR name = 'tiles_data')
                 AND type = 'table'
             --
         ) AND (
             -- 'tiles_shallow' table's columns and their types are as expected:
             -- 4 columns (zoom_level, tile_column, tile_row, tile_data_id).
             -- The order is not important
             SELECT COUNT(*) = 4
             FROM pragma_table_info('tiles_shallow')
             WHERE ((name = 'zoom_level' AND type LIKE '%INT%')
                 OR (name = 'tile_column' AND type LIKE '%INT%')
                 OR (name = 'tile_row' AND type LIKE '%INT%')
                 OR (name = 'tile_data_id' AND type LIKE '%INT%'))
             --
         ) AND (
             -- 'tiles_data' table's columns and their types are as expected:
             -- 2 columns (tile_data_id, tile_data).
             -- The order is not important
             SELECT COUNT(*) = 2
             FROM pragma_table_info('tiles_data')
             WHERE ((name = 'tile_data_id' AND type LIKE '%INT%')
                 OR (name = 'tile_data' AND type = 'BLOB'))
             --
         ) AS is_valid;"
    );

    Ok(sql.fetch_one(&mut *conn).await?.is_valid == 1)
}

pub async fn create_normalized_tables<T>(conn: &mut T, strict: bool) -> MbtResult<()>
where
    for<'e> &'e mut T: SqliteExecutor<'e>,
{
    debug!("Creating if needed normalized tables and tiles view: map(z,x,y,id) + images(id,data)");
    create_schema(conn, include_str!("../../sql/init-normalized.sql"), strict).await
}

/// Temporary table mapping a tile hash to the integer `tile_data_id` it was given.
///
/// The dedup-id schema stores no hashes, so writers keep this connection-scoped side table to
/// recognise a repeated tile. It is seeded from an existing `tiles_data` so that appending to a
/// file reuses its ids instead of storing the same blob twice.
pub(crate) const DEDUP_ID_MAP: &str = "mbt_dedup_ids";

pub(crate) const CREATE_DEDUP_ID_MAP_SQL: &str = "CREATE TEMP TABLE IF NOT EXISTS mbt_dedup_ids (
     tile_hash TEXT PRIMARY KEY,
     tile_data_id INTEGER NOT NULL
 )";

pub(crate) const DEDUP_ID_MAP_EXISTS_SQL: &str =
    "SELECT 1 FROM temp.sqlite_master WHERE type = 'table' AND name = 'mbt_dedup_ids'";

pub(crate) fn seed_dedup_id_map_sql(algorithm: HashAlgorithm) -> String {
    let hash = algorithm.sql_hash("tile_data");
    format!(
        "INSERT OR IGNORE INTO {DEDUP_ID_MAP} (tile_hash, tile_data_id)
         SELECT COALESCE({hash}, ''), tile_data_id FROM tiles_data"
    )
}

/// Create the dedup-id variant of the normalized schema: `tiles_shallow` + `tiles_data`
/// keyed by an integer `tile_data_id`, plus the spec-compatible `tiles` view.
pub async fn create_dedup_id_normalized_tables<T>(conn: &mut T, strict: bool) -> MbtResult<()>
where
    for<'e> &'e mut T: SqliteExecutor<'e>,
{
    debug!(
        "Creating if needed dedup-id normalized tables and tiles view: tiles_shallow(z,x,y,id) + tiles_data(id,data)"
    );
    create_schema(
        conn,
        include_str!("../../sql/init-normalized-dedup-id.sql"),
        strict,
    )
    .await
}

pub async fn create_tiles_with_hash_view<T>(conn: &mut T) -> MbtResult<()>
where
    for<'e> &'e mut T: SqliteExecutor<'e>,
{
    debug!("Creating if needed tiles_with_hash view for normalized map+images structure");
    create_schema(
        conn,
        include_str!("../../sql/init-normalized-with-hash.sql"),
        false,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::anonymous_mbtiles;
    use crate::{has_tiles_with_hash, is_flat_tables_type};

    #[actix_rt::test]
    async fn create_and_detect_normalized() {
        let (_mbt, mut conn) = anonymous_mbtiles("").await;
        create_normalized_tables(&mut conn, false).await.unwrap();

        assert!(is_normalized_tables_type(&mut conn).await.unwrap());
        assert!(!is_dedup_id_normalized_tables_type(&mut conn).await.unwrap());
        // 'tiles' is only a view here, so the flat (table) detection must not match
        assert!(!is_flat_tables_type(&mut conn).await.unwrap());

        // The hash view is optional and created separately
        assert!(!has_tiles_with_hash(&mut conn).await.unwrap());
        create_tiles_with_hash_view(&mut conn).await.unwrap();
        assert!(has_tiles_with_hash(&mut conn).await.unwrap());
    }

    #[actix_rt::test]
    async fn detect_dedup_id_normalized() {
        let script = include_str!("../../../tests/fixtures/mbtiles/normalized-dedup-id.sql");
        let (_mbt, mut conn) = anonymous_mbtiles(script).await;

        assert!(is_dedup_id_normalized_tables_type(&mut conn).await.unwrap());
    }
}
