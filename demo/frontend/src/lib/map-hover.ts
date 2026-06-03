import type { DemoLayerEntry } from '@/content.config';

/**
 * Feature currently under the cursor on the map.
 * `name` is a resolved display label; `properties` carries all raw feature props.
 */
export interface HoveredFeature {
  name: string;
  properties: Record<string, unknown>;
}

/**
 * Build a HoveredFeature from a raw GeoJSON feature returned by MapLibre.
 * The display name is derived from the layer's `hoverNameField` (if set) or
 * falls back to the standard "name" property.
 */
export function buildHoveredFeature(
  src: DemoLayerEntry,
  properties: Record<string, unknown>,
): HoveredFeature {
  const nameField = src.hoverNameField ?? 'name';
  const raw = properties[nameField];
  const name =
    raw !== undefined && raw !== null
      ? nameField === src.hoverNameField && src.hoverNameField !== 'name'
        ? `Zone ${raw}`
        : String(raw)
      : '';

  return { name, properties };
}
