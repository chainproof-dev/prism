// Data palette (docs/08 § 4): 12 anchor hues rendered per theme via OKLCH.
// The canvas never parses CSS — palette resolution happens here from the same
// token values (single source: this file + globals.css blocks stay in sync by
// the token CI check).

export interface DataPalette {
  /** 12 anchor hue colors (category mode). */
  spectrum: string[];
  /** Age ramp: 6 buckets fresh → ancient. */
  ageRamp: string[];
  /** Free-space recessive fill. */
  freeSpace: string;
  /** Unknown hatched fill. */
  unknown: string;
}

function oklch(l: number, c: number, h: number): string {
  return `oklch(${l} ${c} ${h})`;
}

/** Build the palette for a theme (dark themes get brighter data colors). */
export function dataPalette(theme: string): DataPalette {
  const dark = !['alabaster', 'terracotta'].includes(theme);
  // 12 anchors (docs/08 § 4.1): cyan, azure, violet, magenta, rose, coral,
  // amber, lime-green, green, teal, slate-blue, mauve
  const hues = [200, 250, 300, 350, 15, 40, 75, 135, 155, 185, 265, 320];
  const L = dark ? 0.72 : 0.55;
  const C = dark ? 0.14 : 0.12;
  const spectrum = hues.map((h) => oklch(L, C, h));
  const ageRamp = dark
    ? [
        oklch(0.8, 0.15, 155), // 0–7d fresh green
        oklch(0.78, 0.13, 120), // 7–30d
        oklch(0.8, 0.13, 85), // 30–90d
        oklch(0.76, 0.11, 60), // 90d–1y
        oklch(0.66, 0.08, 40), // 1–2y
        oklch(0.52, 0.03, 60), // >2y ancient umber
      ]
    : [
        oklch(0.55, 0.14, 155),
        oklch(0.55, 0.12, 120),
        oklch(0.58, 0.12, 85),
        oklch(0.55, 0.1, 60),
        oklch(0.47, 0.08, 40),
        oklch(0.4, 0.03, 60),
      ];
  return {
    spectrum,
    ageRamp,
    freeSpace: dark ? oklch(0.32, 0.014, 264) : oklch(0.88, 0.006, 250),
    unknown: oklch(dark ? 0.7 : 0.6, 0.12, 85),
  };
}

/** Map a palette-index slot (from a VizFrame) to a concrete color. */
export function resolvePaletteIndex(
  slot: number,
  palette: DataPalette,
  isFree: boolean,
  isUnknown: boolean,
): string {
  if (isFree) {
    return palette.freeSpace;
  }
  if (isUnknown) {
    return palette.unknown;
  }
  const kindBits = slot & 0xffff0000;
  const index = slot & 0xffff;
  if (kindBits === 0x20000) {
    return palette.spectrum[index % palette.spectrum.length]!;
  }
  if (kindBits === 0x30000) {
    return palette.ageRamp[index % palette.ageRamp.length]!;
  }
  // category (0x10000): spread the ~34 category ids over the 12 anchors
  return palette.spectrum[index % palette.spectrum.length]!;
}

/** Parse an oklch() string into a canvas-ready rgb() via CSS round-trip. */
export function toCanvasColor(cssColor: string): string {
  return cssColor; // Chromium canvas accepts oklch() directly
}

export const THEMES = ['nocturne', 'graphite', 'verdigris', 'ember', 'alabaster', 'terracotta'] as const;
export type ThemeName = (typeof THEMES)[number];
