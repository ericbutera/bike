import type { HeatmapViewAction } from "../lib/heatmapCamera";

export default function HeatmapZoomPresets({
  onChoose,
  canFit,
}: {
  onChoose: (action: HeatmapViewAction) => void;
  canFit: boolean;
}) {
  return (
    <details className="dropdown dropdown-end">
      <summary className="btn btn-sm btn-ghost">Zoom preset</summary>
      <div className="dropdown-content z-30 mt-2 w-40 rounded-box border border-base-300 bg-base-100 p-2 shadow-xl">
        {(["region", "full"] as const).map((action) => (
          <button
            key={action}
            className="btn btn-sm btn-ghost w-full justify-start"
            disabled={action === "full" && !canFit}
            onClick={(event) => {
              event.currentTarget.closest("details")?.removeAttribute("open");
              onChoose(action);
            }}
          >
            {action === "region" ? "Region" : "Full"}
          </button>
        ))}
      </div>
    </details>
  );
}
