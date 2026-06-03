-- Index supporting get_flows: per-zone range scan on pickup_datetime.
create index if not exists idx_trips_pu_pickup on trips (pulocationid, pickup_datetime);

drop function if exists get_flows(integer, integer, integer, json);

create or replace function get_flows(
    z integer, x integer, y integer, query_params json
) returns bytea
stable
strict
parallel safe
language plpgsql
as $$
declare
  bounds GEOMETRY(POLYGON, 3857) := TileBBox(z, x, y, 3857);
  date_from DATE := (query_params->>'date_from')::DATE;
  date_to DATE := (query_params->>'date_to')::DATE;
  in_hour INTEGER := (query_params->>'hour')::INTEGER;
  min_trips INTEGER := COALESCE(NULLIF(query_params->>'min_trips', '')::INTEGER, 25);
  res BYTEA;
begin
  WITH pickup_zones AS (
    SELECT locationid, ST_Centroid(geom) AS centroid
    FROM taxi_zones
    WHERE geom && bounds
  ),
  dropoff_zones AS (
    SELECT locationid, ST_Centroid(geom) AS centroid
    FROM taxi_zones
  ),
  flows AS (
    SELECT
      t.pulocationid,
      t.dolocationid,
      COUNT(*) AS trips
    FROM trips t
    WHERE t.pulocationid IN (SELECT locationid FROM pickup_zones)
      AND t.pickup_datetime >= date_from
      AND t.pickup_datetime <  date_to + INTERVAL '1 day'
      AND (in_hour = -1 OR EXTRACT(HOUR FROM t.pickup_datetime) = in_hour)
      AND t.dolocationid IS NOT NULL
      AND t.pulocationid != t.dolocationid
    GROUP BY t.pulocationid, t.dolocationid
    HAVING COUNT(*) >= min_trips
  ),
  tile AS (
    SELECT
      f.pulocationid::integer AS pu_id,
      f.dolocationid::integer AS do_id,
      f.trips::integer        AS trips,
      ST_AsMVTGeom(
        ST_MakeLine(pz.centroid, dz.centroid),
        bounds, 4096, 1024, TRUE
      ) AS geom
    FROM flows f
    JOIN pickup_zones pz ON f.pulocationid = pz.locationid
    JOIN dropoff_zones dz ON f.dolocationid = dz.locationid
  )
  SELECT INTO res ST_AsMVT(tile, 'flows', 4096, 'geom')
  FROM tile
  WHERE geom IS NOT NULL;

  RETURN res;
END;
$$;

alter function get_flows(integer, integer, integer, json) owner to postgres;
