import { Layers } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import BottomSheet from '@/components/BottomSheet';
import type { FilterState } from '@/components/demo/FilterPanel';
import FilterPanel, { getInitialFilterState } from '@/components/demo/FilterPanel';
import MetricsPanel from '@/components/demo/MetricsPanel';
import type { DemoScenarioItem } from '@/components/demo/ScenarioSelector';
import ScenarioSelector from '@/components/demo/ScenarioSelector';
import StylingPanel from '@/components/demo/StylingPanel';
import TimeSlider from '@/components/demo/TimeSlider';
import type { MapStylingOptions } from '@/components/martin-map';
import MartinMap from '@/components/martin-map';
import type { DemoLayerEntry } from '@/content.config';
import type { HoveredFeature } from '@/lib/map-hover';
import { useMediaQueryMinHeight, useMediaQuerySm } from '@/lib/useMediaQuery';

interface MapDemoProps {
  tileSources: DemoLayerEntry[];
  demoScenarios: DemoScenarioItem[];
  martinBaseUrl: string;
}

function HoverInfo({ hovered }: { hovered: HoveredFeature | null }) {
  if (hovered == null) return null;
  const trips = hovered.properties.trips;
  const pop = hovered.properties.pop_est;
  let suffix = '';
  if (trips != null) suffix = ` · ${trips} trips`;
  else if (pop != null) suffix = ` · ${((pop as number) / 1_000_000).toFixed(1)}M`;
  return (
    <span className="text-[10px] font-mono text-muted-foreground truncate max-w-[180px]">
      {hovered.name}
      {suffix}
    </span>
  );
}

export default function MapDemo({ tileSources, demoScenarios, martinBaseUrl }: MapDemoProps) {
  const [activeLayer, setActiveLayer] = useState(tileSources[0]?.id ?? '');
  const activeLayerConfig = tileSources.find((s) => s.id === activeLayer) ?? null;
  const [filterState, setFilterState] = useState<FilterState>(() =>
    getInitialFilterState(activeLayerConfig),
  );
  const [hovered, setHovered] = useState<HoveredFeature | null>(null);
  const [styling, setStyling] = useState<MapStylingOptions>({
    fontStack: 'default',
    spriteType: 'sdf',
    styleId: 'dark',
  });
  const [mobileSheetOpen, setMobileSheetOpen] = useState(false);
  const isWide = useMediaQuerySm();
  const isTall = useMediaQueryMinHeight();
  const isDesktop = isWide && isTall;

  useEffect(() => {
    setFilterState(getInitialFilterState(activeLayerConfig));
  }, [activeLayerConfig]);

  const handleFilterChange = useCallback((name: string, value: string | number) => {
    setFilterState((prev) => ({ ...prev, [name]: value }));
  }, []);

  const handleScenarioSelect = useCallback((preset: Record<string, string | number>) => {
    setFilterState((prev) => ({ ...prev, ...preset }));
  }, []);

  const layerTabs = (
    <div className="flex flex-wrap gap-1">
      {tileSources.map((s) => (
        <button
          className={`px-2 py-0.5 text-[11px] font-mono rounded transition-colors ${
            activeLayer === s.id
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:text-foreground border border-border'
          }`}
          key={s.id}
          onClick={() => setActiveLayer(s.id)}
          type="button"
        >
          {s.label}
        </button>
      ))}
    </div>
  );

  const stylingControls = (
    <StylingPanel
      fontStack={styling.fontStack}
      onFontChange={(fontStack) => setStyling((s) => ({ ...s, fontStack }))}
      onSpriteTypeChange={(spriteType) => setStyling((s) => ({ ...s, spriteType }))}
      onStyleChange={(styleId) => setStyling((s) => ({ ...s, styleId }))}
      spriteType={styling.spriteType}
      styleId={styling.styleId}
    />
  );

  const controlsBody = (
    <>
      <div className="flex flex-wrap items-end gap-x-4 gap-y-2">
        <FilterPanel
          filterState={filterState}
          layer={activeLayerConfig}
          onFilterChange={handleFilterChange}
        />
        <TimeSlider
          filterState={filterState}
          layer={activeLayerConfig}
          onFilterChange={handleFilterChange}
        />
      </div>
      <ScenarioSelector
        activeLayerId={activeLayer}
        filterState={filterState}
        onSelectScenario={handleScenarioSelect}
        scenarios={demoScenarios}
      />
    </>
  );

  return (
    <div className="absolute inset-0">
      <MartinMap
        activeLayer={activeLayer}
        filterState={filterState}
        martinBaseUrl={martinBaseUrl}
        onActiveLayerChange={setActiveLayer}
        onHoveredChange={setHovered}
        styling={styling}
        tileSources={tileSources}
      />

      {isDesktop && (
        <div className="absolute bottom-2 left-1/2 z-10 w-[min(820px,calc(100%-2rem))] -translate-x-1/2 pointer-events-auto">
          <div className="bg-background/95 backdrop-blur-md border border-border rounded-lg shadow-lg overflow-hidden">
            <div className="flex flex-wrap items-center gap-x-3 gap-y-2 px-3 py-2 border-b border-border">
              {layerTabs}
              <div className="ml-auto">{stylingControls}</div>
            </div>
            <div className="px-3 py-2 flex flex-col gap-2 max-h-[min(40vh,320px)] overflow-y-auto">
              {controlsBody}
            </div>
            <div className="flex items-center justify-end gap-3 px-3 py-1.5 border-t border-border">
              <HoverInfo hovered={hovered} />
              <MetricsPanel martinBaseUrl={martinBaseUrl} />
            </div>
          </div>
        </div>
      )}

      {!isDesktop && (
        <>
          <button
            aria-label="Open demo controls"
            className="absolute bottom-4 left-1/2 z-10 -translate-x-1/2 pointer-events-auto flex items-center gap-2 px-4 py-3 text-sm font-mono bg-background/95 backdrop-blur-md border border-border rounded-lg shadow-lg text-foreground hover:border-primary/50 transition-colors"
            onClick={() => setMobileSheetOpen(true)}
            type="button"
          >
            <Layers className="size-4 shrink-0" />
            Layers & filters
          </button>
          <BottomSheet
            onClose={() => setMobileSheetOpen(false)}
            open={mobileSheetOpen}
            title="Demo controls"
          >
            <div className="p-4 flex flex-col gap-4">
              <div className="flex flex-wrap items-center gap-2">
                {layerTabs}
                <div className="ml-auto">{stylingControls}</div>
              </div>
              <div className="flex flex-col gap-2">{controlsBody}</div>
              <div className="flex items-center justify-end gap-3 pt-2 border-t border-border">
                <MetricsPanel martinBaseUrl={martinBaseUrl} />
              </div>
            </div>
          </BottomSheet>
        </>
      )}
    </div>
  );
}
