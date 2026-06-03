interface StylingPanelProps {
  spriteType: 'sdf' | 'plain';
  styleId: string;
  fontStack: string;
  onSpriteTypeChange: (value: 'sdf' | 'plain') => void;
  onStyleChange: (value: string) => void;
  onFontChange: (value: string) => void;
}

const STYLE_OPTIONS = [
  { id: 'dark', label: 'Toner' },
  { id: 'light', label: 'Positron' },
] as const;

const FONT_OPTIONS = [
  { id: 'default', label: 'Default' },
  { id: 'noto', label: 'Noto Sans' },
] as const;

const selectClass =
  'rounded border border-border bg-background px-1.5 py-0.5 text-[11px] font-mono text-foreground';

export default function StylingPanel({
  spriteType,
  styleId,
  fontStack,
  onSpriteTypeChange,
  onStyleChange,
  onFontChange,
}: StylingPanelProps) {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] font-mono text-muted-foreground">
      <label className="flex items-center gap-1">
        style
        <select
          aria-label="Map style"
          className={selectClass}
          onChange={(e) => onStyleChange(e.target.value)}
          value={styleId}
        >
          {STYLE_OPTIONS.map((opt) => (
            <option key={opt.id} value={opt.id}>
              {opt.label}
            </option>
          ))}
        </select>
      </label>
      <label className="flex items-center gap-1">
        font
        <select
          aria-label="Font stack"
          className={selectClass}
          onChange={(e) => onFontChange(e.target.value)}
          value={fontStack}
        >
          {FONT_OPTIONS.map((opt) => (
            <option key={opt.id} value={opt.id}>
              {opt.label}
            </option>
          ))}
        </select>
      </label>
      <label className="flex items-center gap-1">
        sprite
        <select
          aria-label="Sprite type"
          className={selectClass}
          onChange={(e) => onSpriteTypeChange(e.target.value as 'sdf' | 'plain')}
          value={spriteType}
        >
          <option value="sdf">SDF</option>
          <option value="plain">Plain</option>
        </select>
      </label>
    </div>
  );
}
