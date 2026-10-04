import type { RasterLayerSpecification } from "maplibre-gl";

export const HEATMAP_COLOR_STORAGE_KEY = "bike-heatmap-color";

export const HEATMAP_PALETTES = [
  { id: "orange", label: "Orange", hue: 180, contrast: 0, brightness: 1 },
  { id: "sunset", label: "Sunset", hue: 145, contrast: 0, brightness: 1 },
  { id: "blue", label: "Blue", hue: 0, contrast: 0, brightness: 1 },
  { id: "pink", label: "Pink", hue: 120, contrast: 0, brightness: 1 },
  { id: "purple", label: "Purple", hue: 60, contrast: 0, brightness: 1 },
  {
    id: "contrast",
    label: "Contrast",
    hue: 20,
    contrast: 0.5,
    brightness: 0.85,
  },
] as const;

export type HeatmapPaletteId = (typeof HEATMAP_PALETTES)[number]["id"];
type HeatmapPalette = (typeof HEATMAP_PALETTES)[number];

export function heatmapPalette(value?: string | null): HeatmapPalette {
  return (
    HEATMAP_PALETTES.find((palette) => palette.id === value) ??
    HEATMAP_PALETTES[2]
  );
}

export function heatmapPalettePaint(
  palette: HeatmapPalette,
): NonNullable<RasterLayerSpecification["paint"]> {
  return {
    "raster-hue-rotate": palette.hue,
    "raster-contrast": palette.contrast,
    "raster-brightness-max": palette.brightness,
  };
}

/** Match MapLibre's raster shader so the picker and legend show the same color
 * as the #0060df heatmap-v2 tiles. Opacity remains encoded in the tile alpha. */
export function heatmapPaletteColor(palette: HeatmapPalette): string {
  const angle = (palette.hue * Math.PI) / 180;
  const sin = Math.sin(angle);
  const cos = Math.cos(angle);
  const weights = [
    (2 * cos + 1) / 3,
    (-Math.sqrt(3) * sin - cos + 1) / 3,
    (Math.sqrt(3) * sin - cos + 1) / 3,
  ];
  const source = [0, 96 / 255, 223 / 255];
  const contrast = 1 / (1 - palette.contrast);
  const rgb = [0, 2, 1].map((offset) => {
    const rotated = source.reduce(
      (sum, channel, index) => sum + channel * weights[(index + offset) % 3],
      0,
    );
    const adjusted = ((rotated - 0.5) * contrast + 0.5) * palette.brightness;
    return Math.round(Math.max(0, Math.min(1, adjusted)) * 255);
  });
  return `rgb(${rgb.join(", ")})`;
}
