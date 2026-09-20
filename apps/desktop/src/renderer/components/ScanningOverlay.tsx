// Scanning overlay (docs/10 § 4): live counters (≤ 4 Hz from the engine),
// current path, cancel/pause, luminance-sweep motif (parity-SCN-03 substitution).
import { useEffect, useRef } from 'react';
import { Pause, Play, X } from 'lucide-react';
import { useScanStore } from '../stores/scan';
import { cancelScan, pauseScan, resumeScan } from '../lib/prism';
import { formatBytes, formatDuration, formatRate } from '@prism/shared/client';

export function ScanningOverlay(): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const progress = useScanStore((s) => s.progress);
  const phase = useScanStore((s) => s.phase);
  const liveTree = useScanStore((s) => s.liveTree);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // luminance sweep motif (canvas, GPU-composited, zero layout cost)
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }
    let raf = 0;
    let t = 0;
    const draw = (): void => {
      const { width, height } = canvas;
      ctx.clearRect(0, 0, width, height);
      t += 0.008;
      const x = (Math.sin(t) * 0.5 + 0.5) * width;
      const grad = ctx.createLinearGradient(x - 160, 0, x + 160, 0);
      grad.addColorStop(0, 'transparent');
      grad.addColorStop(0.5, 'rgba(83, 215, 240, 0.05)');
      grad.addColorStop(1, 'transparent');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, width, height);
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, []);

  const files = Number(progress?.filesSeen ?? 0);
  const dirs = Number(progress?.dirsSeen ?? 0);
  const bytes = progress?.bytesSeen ?? 0n;
  const elapsed = Number(progress?.elapsedMs ?? 0);

  return (
    <div className="relative flex h-full flex-col items-center justify-center gap-8">
      <canvas ref={canvasRef} className="pointer-events-none absolute inset-0 h-full w-full" />
      <div className="relative z-10 text-center">
        <div className="micro-label mb-2">
          {phase === 'walking' ? 'Walking directories' : phase === 'aggregating' ? 'Aggregating' : phase === 'indexing-ext' ? 'Indexing types' : phase}
        </div>
        <div className="mono text-[34px] leading-10 font-medium text-text-primary">{formatBytes(bytes)}</div>
        <div className="mono mt-2 text-xs text-text-secondary">
          {files.toLocaleString('en-US')} files · {dirs.toLocaleString('en-US')} folders · {formatRate(progress?.rateFilesPerSec ?? 0)}
        </div>
        <div className="mono mt-1 text-xs text-text-faint">
          {liveTree.size.toLocaleString('en-US')} nodes · {formatDuration(elapsed)} elapsed
        </div>
        <div className="mono mt-4 max-w-[560px] truncate text-xs text-text-faint">{progress?.currentPath ?? ''}</div>
      </div>
      <div className="relative z-10 flex gap-2">
        <button
          onClick={() => (phase === 'paused' && scanId !== null ? void resumeScan(scanId) : scanId !== null ? void pauseScan(scanId) : undefined)}
          className="flex items-center gap-1.5 rounded-sm border border-hairline bg-surface-raised px-3 py-1.5 text-xs hover:border-border-strong"
        >
          {phase === 'paused' ? <Play size={12} /> : <Pause size={12} />}
          {phase === 'paused' ? 'Resume' : 'Pause'}
        </button>
        <button
          onClick={() => (scanId !== null ? void cancelScan(scanId) : undefined)}
          className="flex items-center gap-1.5 rounded-sm border border-hairline bg-surface-raised px-3 py-1.5 text-xs hover:border-border-strong"
        >
          <X size={12} /> Cancel <span className="text-text-faint">Esc</span>
        </button>
      </div>
    </div>
  );
}
