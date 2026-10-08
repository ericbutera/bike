import type { HeatmapMetadata } from "../lib/heatmaps";

const LEGEND = [
  { label: "1", opacity: 100 / 255 },
  { label: "2–4", opacity: 155 / 255 },
  { label: "5–9", opacity: 195 / 255 },
  { label: "10–24", opacity: 225 / 255 },
  { label: "25+", opacity: 1 },
];

export default function HeatmapHelp({
  metadata,
  color,
}: {
  metadata?: HeatmapMetadata;
  color: string;
}) {
  return (
    <details className="dropdown dropdown-end">
      <summary
        className="btn btn-circle btn-sm btn-ghost"
        aria-label="Heatmap help and legend"
        title="Heatmap help and legend"
      >
        <span
          aria-hidden="true"
          className="flex size-5 items-center justify-center rounded-full border border-current text-xs font-bold"
        >
          ?
        </span>
      </summary>
      <div className="dropdown-content z-30 mt-2 w-[min(90vw,24rem)] rounded-box border border-base-300 bg-base-100 p-4 shadow-xl">
        <h2 className="font-semibold">Heatmap legend</h2>
        <p className="mt-1 text-sm opacity-70">
          Paths become more opaque as more of your activities use them. Line
          width and your selected color stay the same.
        </p>
        <p className="mt-2 text-sm opacity-70">
          At low zoom, circles group route locations. Their number is the
          activities in that area.
        </p>
        <div
          className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm"
          aria-label="Activities per path legend"
        >
          <span className="opacity-70">Per path</span>
          {LEGEND.map((item) => (
            <span key={item.label} className="flex items-center gap-1.5">
              <span
                aria-hidden="true"
                style={{
                  display: "inline-block",
                  width: 18,
                  height: 2,
                  background: color,
                  opacity: item.opacity,
                  borderRadius: 8,
                }}
              />
              {item.label}
            </span>
          ))}
        </div>
        {metadata && (
          <p className="mt-3 text-sm">
            {metadata.ready.toLocaleString()} activities with routes
          </p>
        )}
      </div>
    </details>
  );
}
