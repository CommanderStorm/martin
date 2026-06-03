import type { FilterState } from './FilterPanel';

export interface DemoScenarioItem {
  id: string;
  data: { label: string; layerId: string; preset: Record<string, string | number> };
}

interface ScenarioSelectorProps {
  scenarios: DemoScenarioItem[];
  activeLayerId: string;
  filterState: FilterState;
  onSelectScenario: (preset: Record<string, string | number>) => void;
}

export default function ScenarioSelector({
  scenarios,
  activeLayerId,
  onSelectScenario,
}: ScenarioSelectorProps) {
  const forLayer = scenarios.filter((s) => s.data.layerId === activeLayerId);
  if (forLayer.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1">
      <span className="text-[10px] font-mono text-muted-foreground mr-1">presets</span>
      {forLayer.map((entry) => (
        <button
          className="rounded border border-border bg-background px-2 py-0.5 text-[11px] font-mono text-foreground hover:border-primary/50 hover:bg-muted/50"
          key={entry.id}
          onClick={() => onSelectScenario(entry.data.preset)}
          type="button"
        >
          {entry.data.label}
        </button>
      ))}
    </div>
  );
}
