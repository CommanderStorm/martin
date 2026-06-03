import { useCallback, useEffect, useRef, useState } from 'react';
import { aggregateTileMetrics, parsePrometheusMetrics } from '@/lib/prometheus';

interface MetricsPanelProps {
  martinBaseUrl: string;
  refreshIntervalMs?: number;
}

interface MetricsData {
  requestCount: number;
  averageDurationMs: number;
}

export default function MetricsPanel({
  martinBaseUrl,
  refreshIntervalMs = 5000,
}: MetricsPanelProps) {
  const [metrics, setMetrics] = useState<MetricsData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fetchMetricsRef = useRef<(() => Promise<void>) | null>(null);

  const fetchMetrics = useCallback(async () => {
    const url = `${martinBaseUrl.replace(/\/$/, '')}/_/metrics`;
    try {
      const res = await fetch(url);
      if (!res.ok) {
        setError(`HTTP ${res.status}`);
        return;
      }
      const text = await res.text();
      const { sum, count } = parsePrometheusMetrics(text);
      const tile = aggregateTileMetrics(sum, count);
      setMetrics({
        averageDurationMs: tile.averageDurationMs,
        requestCount: tile.requestCount,
      });
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to fetch');
    }
  }, [martinBaseUrl]);

  useEffect(() => {
    fetchMetricsRef.current = fetchMetrics;
  }, [fetchMetrics]);

  useEffect(() => {
    fetchMetrics();
    const id = setInterval(() => fetchMetricsRef.current?.(), refreshIntervalMs);
    return () => clearInterval(id);
  }, [fetchMetrics, refreshIntervalMs]);

  if (error && !metrics) {
    return <span className="text-[10px] font-mono text-muted-foreground">metrics: {error}</span>;
  }

  if (!metrics) {
    return <span className="text-[10px] font-mono text-muted-foreground">metrics: -</span>;
  }

  const stale = error != null;
  return (
    <span
      className={`text-[10px] font-mono ${stale ? 'text-amber-400' : 'text-muted-foreground'}`}
    >
      {metrics.requestCount.toLocaleString()} tile requests · {metrics.averageDurationMs.toFixed(1)}{' '}
      ms avg
      {stale && ' (stale)'}
    </span>
  );
}
