"use client";

import { useRef } from "react";
import {
  HEATMAP_PALETTES,
  heatmapPalette,
  heatmapPaletteColor,
  type HeatmapPaletteId,
} from "../lib/heatmapColors";

export default function HeatmapColorPicker({
  value,
  onChange,
}: {
  value: HeatmapPaletteId;
  onChange: (value: HeatmapPaletteId) => void;
}) {
  const details = useRef<HTMLDetailsElement>(null);
  const selected = heatmapPalette(value);
  return (
    <details ref={details} className="dropdown dropdown-end">
      <summary
        className="btn btn-sm btn-ghost"
        aria-label={`Heatmap color: ${selected.label}`}
      >
        <span
          aria-hidden="true"
          className="size-3 rounded-full"
          style={{ background: heatmapPaletteColor(selected) }}
        />
        Color
      </summary>
      <fieldset
        className="dropdown-content z-20 mt-2 w-72 rounded-box border border-base-300 bg-base-100 p-4 shadow-xl"
        aria-label="Heatmap line color"
      >
        <legend className="sr-only">Heatmap line color</legend>
        <p className="mb-3 font-semibold">Color</p>
        <div className="grid grid-cols-2 gap-2">
          {HEATMAP_PALETTES.map((palette) => {
            const color = heatmapPaletteColor(palette);
            return (
              <label
                key={palette.id}
                className="flex cursor-pointer items-center gap-2 rounded-lg p-2 hover:bg-base-200"
              >
                <input
                  className="peer sr-only"
                  type="radio"
                  name="heatmap-color"
                  value={palette.id}
                  checked={value === palette.id}
                  onChange={() => {
                    onChange(palette.id);
                    details.current?.removeAttribute("open");
                  }}
                />
                <span
                  aria-hidden="true"
                  className="size-7 shrink-0 rounded-full border-2 border-base-100 ring-1 ring-base-300 peer-checked:ring-2 peer-checked:ring-primary peer-focus-visible:outline-2 peer-focus-visible:outline-offset-4 peer-focus-visible:outline-primary"
                  style={{
                    background: `linear-gradient(90deg, ${color}, color-mix(in srgb, ${color} 30%, transparent))`,
                  }}
                />
                <span className="text-sm peer-checked:font-semibold">
                  {palette.label}
                </span>
              </label>
            );
          })}
        </div>
      </fieldset>
    </details>
  );
}
