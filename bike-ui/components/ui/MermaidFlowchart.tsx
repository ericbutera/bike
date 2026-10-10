"use client";
import { useEffect, useId, useRef, useState } from "react";

export default function MermaidFlowchart({
  chart,
  onSelect,
}: {
  chart: string;
  onSelect: (stage: string) => void;
}) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    async function render() {
      try {
        setError(null);
        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: "base",
        });
        const { svg } = await mermaid.render(
          `activity-import-${id}-${Date.now()}`,
          chart,
        );
        if (!cancelled && containerRef.current)
          containerRef.current.innerHTML = svg;
      } catch (error) {
        if (!cancelled)
          setError(
            error instanceof Error
              ? error.message
              : "Failed to render import graph",
          );
      }
    }
    if (chart) void render();
    return () => {
      cancelled = true;
    };
  }, [chart, id]);
  return (
    <div className="overflow-x-auto rounded-box border border-base-300 p-4">
      <div
        ref={containerRef}
        onClick={(event) => {
          const node = (event.target as Element).closest("g.node");
          const stage = node?.id.match(/flowchart-(.+)-\d+$/)?.[1];
          if (stage) onSelect(stage);
        }}
      />
      {error ? <p className="text-error">{error}</p> : null}
    </div>
  );
}
