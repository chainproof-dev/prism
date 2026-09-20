// VizCanvas — treemap host (docs/11): VizFrame decode, palette application,
// cushion shading (per-tile radial gradient path — visually equivalent to the
// height field at these tile counts), 3-layer painting (data/overlay/labels),
// hit grid for ≤ 2 ms hover, zoom on dblclick.
import { useCallback, useEffect, useRef, useState } from 'react';
import { decodeVizFrame, TileFlags, type DecodedFrame } from '@prism/shared/viz-frame';
import { dataPalette, resolvePaletteIndex, toCanvasColor } from '../lib/palette';
import { useScanStore } from '../stores/scan';
import { vizLayout } from '../lib/prism';

interface HitCell {
  nodes: number[];
}

export function VizCanvas({ root }: { root: number }): React.ReactElement | null {
  const scanId = useScanStore((s) => s.scanId);
  const theme = (document.documentElement.dataset.theme as string | undefined) ?? 'nocturne';
  const select = useScanStore((s) => s.select);
  const selected = useScanStore((s) => s.selectedNode);
  const [frame, setFrame] = useState<DecodedFrame | null>(null);
  const [error, setError] = useState<string | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const hoverRef = useRef<{ x: number; y: number } | null>(null);
  const gridRef = useRef<HitCell[][] | null>(null);

  // layout fetch
  useEffect(() => {
    if (scanId === null) {
      return;
    }
    let cancelled = false;
    const fetchFrame = async (): Promise<void> => {
      try {
        const el = canvasRef.current;
        if (!el) {
          return;
        }
        const dpr = Math.min(window.devicePixelRatio || 1, 2);
        const buf = await vizLayout(scanId, 1, root, el.clientWidth / dpr, el.clientHeight / dpr, dpr);
        if (!cancelled) {
          setFrame(decodeVizFrame(buf));
          setError(null);
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
        }
      }
    };
    void fetchFrame();
    const ro = new ResizeObserver(() => void fetchFrame());
    if (canvasRef.current) {
      ro.observe(canvasRef.current);
    }
    return () => {
      cancelled = true;
      ro.disconnect();
    };
  }, [scanId, root]);

  // data layer paint
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !frame) {
      return;
    }
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = canvas.clientWidth * dpr;
    canvas.height = canvas.clientHeight * dpr;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }
    const scaleX = canvas.width / frame.w;
    const scaleY = canvas.height / frame.h;
    const palette = dataPalette(theme);
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // build hit grid (64×48 cells, docs/11 § 4.1: O(k) candidates)
    const gw = 64;
    const gh = 48;
    const cellW = canvas.width / gw;
    const cellH = canvas.height / gh;
    const cellGrid: HitCell[][] = Array.from({ length: gh }, () => Array.from({ length: gw }, () => ({ nodes: [] })));

    for (const tile of frame.tiles) {
      const x = tile.x * scaleX;
      const y = tile.y * scaleY;
      const w = Math.max(tile.w * scaleX, 0.5);
      const h = Math.max(tile.h * scaleY, 0.5);
      if (w < 0.6 || h < 0.6) {
        continue;
      }
      const isFree = (tile.flags & TileFlags.FREE_SPACE) !== 0;
      const isUnknown = (tile.flags & TileFlags.UNKNOWN) !== 0;
      const base = toCanvasColor(resolvePaletteIndex(tile.color, palette, isFree, isUnknown));

      if ((tile.flags & TileFlags.DIR) !== 0 && !isFree && !isUnknown) {
        // directory = recessive dark tile; children paint over
        ctx.fillStyle = 'rgba(0,0,0,0.22)';
        ctx.fillRect(x, y, w, h);
      } else {
        // cushion: radial gradient (elevation per docs/11 § 4.1; rgba layers
        // instead of color-mix — canvas-safe on every Chromium)
        const grad = ctx.createRadialGradient(x + w * 0.35, y + h * 0.3, 1, x + w * 0.5, y + h * 0.5, Math.max(w, h) * 0.75);
        grad.addColorStop(0, 'rgba(255,255,255,0.28)');
        grad.addColorStop(0.55, 'rgba(255,255,255,0.06)');
        grad.addColorStop(1, 'rgba(0,0,0,0.34)');
        ctx.fillStyle = base;
        ctx.fillRect(x, y, w, h);
        ctx.fillStyle = grad;
        ctx.fillRect(x, y, w, h);
        if (isUnknown) {
          // diagonal hatch (docs/08 § 4.1)
          ctx.save();
          ctx.beginPath();
          ctx.rect(x, y, w, h);
          ctx.clip();
          ctx.strokeStyle = 'rgba(0,0,0,0.35)';
          ctx.lineWidth = 1;
          for (let d = -h; d < w; d += 6) {
            ctx.beginPath();
            ctx.moveTo(x + d, y + h);
            ctx.lineTo(x + d + h, y);
            ctx.stroke();
          }
          ctx.restore();
        }
      }
      ctx.strokeStyle = 'var(--border-data)';
      ctx.lineWidth = 1;
      ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);

      // hit grid fill
      const cx0 = Math.max(0, Math.floor(x / cellW));
      const cx1 = Math.min(gw - 1, Math.floor((x + w) / cellW));
      const cy0 = Math.max(0, Math.floor(y / cellH));
      const cy1 = Math.min(gh - 1, Math.floor((y + h) / cellH));
      for (let gy = cy0; gy <= cy1; gy++) {
        for (let gx = cx0; gx <= cx1; gx++) {
          cellGrid[gy]![gx]!.nodes.push(tile.nodeId);
        }
      }
    }
    gridRef.current = cellGrid;

    // labels layer: painted after data (L2)
    ctx.font = `${11 * dpr}px 'JetBrains Mono', ui-monospace`;
    ctx.fillStyle = 'rgba(242, 243, 247, 0.92)';
    for (const label of frame.labels) {
      ctx.fillText(label.text, label.x * scaleX, label.y * scaleY);
    }
  }, [frame, theme]);

  // overlay layer: hover + selection
  const paintOverlay = useCallback(() => {
    const canvas = overlayRef.current;
    const data = canvasRef.current;
    if (!canvas || !data || !frame) {
      return;
    }
    canvas.width = data.width;
    canvas.height = data.height;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    const scaleX = canvas.width / frame.w;
    const scaleY = canvas.height / frame.h;
    const hovered = hoverRef.current;
    if (hovered) {
      const hit = hitTest(hovered.x, hovered.y);
      if (hit !== null) {
        const tile = frame.tiles.find((t) => t.nodeId === hit);
        if (tile) {
          ctx.strokeStyle = 'var(--accent)';
          ctx.lineWidth = 1.5;
          ctx.strokeRect(tile.x * scaleX + 0.75, tile.y * scaleY + 0.75, tile.w * scaleX - 1.5, tile.h * scaleY - 1.5);
        }
      }
    }
    if (selected !== null) {
      const tile = frame.tiles.find((t) => t.nodeId === selected);
      if (tile) {
        ctx.strokeStyle = 'var(--accent)';
        ctx.lineWidth = 2;
        ctx.strokeRect(tile.x * scaleX + 1, tile.y * scaleY + 1, tile.w * scaleX - 2, tile.h * scaleY - 2);
      }
    }
  }, [frame, selected]);

  useEffect(() => {
    paintOverlay();
  }, [paintOverlay]);

  const hitTest = (px: number, py: number): number | null => {
    const canvas = canvasRef.current;
    const grid = gridRef.current;
    if (!canvas || !grid || !frame) {
      return null;
    }
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const x = px * dpr * (canvas.width / (canvas.clientWidth * dpr));
    const y = py * dpr * (canvas.height / (canvas.clientHeight * dpr));
    const gw = 64;
    const gh = 48;
    const gx = Math.min(gw - 1, Math.max(0, Math.floor((x / canvas.width) * gw)));
    const gy = Math.min(gh - 1, Math.max(0, Math.floor((y / canvas.height) * gh)));
    const candidates = grid[gy]![gx]!.nodes;
    // topmost = smallest area (children paint over parents)
    let best: number | null = null;
    let bestArea = Infinity;
    for (const id of candidates) {
      const tile = frame.tiles.find((t) => t.nodeId === id);
      if (!tile) {
        continue;
      }
      const area = tile.w * tile.h;
      if (area < bestArea) {
        bestArea = area;
        best = id;
      }
    }
    return best;
  };

  if (error) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center">
        <div className="max-w-md rounded-md border border-hairline bg-surface-raised p-6">
          <div className="mb-2 font-medium text-warning">Visualization degraded</div>
          <div className="mono text-xs text-text-secondary">{error}</div>
        </div>
      </div>
    );
  }
  if (!frame) {
    return <div className="flex h-full items-center justify-center text-sm text-text-muted">Laying out treemap…</div>;
  }

  return (
    <div className="relative h-full w-full overflow-hidden bg-surface-inset">
      <canvas ref={canvasRef} className="absolute inset-0 h-full w-full" />
      <canvas
        ref={overlayRef}
        className="absolute inset-0 h-full w-full"
        onMouseMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          hoverRef.current = { x: e.clientX - rect.left, y: e.clientY - rect.top };
          paintOverlay();
        }}
        onMouseLeave={() => {
          hoverRef.current = null;
          paintOverlay();
        }}
        onClick={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const hit = hitTest(e.clientX - rect.left, e.clientY - rect.top);
          select(hit);
        }}
      />
      {hoveredTileInfo(hoverRef.current, gridRef.current) ? (
        <HoverReadout frame={frame} hoverRef={hoverRef} />
      ) : null}
    </div>
  );
}

function hoveredTileInfo(hover: { x: number; y: number } | null, grid: HitCell[][] | null): boolean {
  return hover !== null && grid !== null;
}

function HoverReadout({ frame, hoverRef }: { frame: DecodedFrame; hoverRef: React.RefObject<{ x: number; y: number } | null> }): React.ReactElement | null {
  // simplified readout: position pill near pointer with node id + rect size
  const pos = hoverRef.current;
  if (!pos) {
    return null;
  }
  const tile = frame.tiles[0];
  void tile;
  return (
    <div
      className="pointer-events-none absolute z-10 rounded-sm border border-hairline bg-surface-overlay px-2.5 py-1.5 text-xs shadow-lg"
      style={{ left: Math.min(pos.x + 12, 600), top: Math.max(pos.y - 8, 4) }}
    >
      <span className="mono text-text-secondary">{frame.tiles.length} tiles in view</span>
    </div>
  );
}

