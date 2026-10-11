"use client";

import { useId } from "react";
import { ProcessingStatus } from "./ProcessingDetails";

type Node = { id: string; label: string; status: string; summary?: string[] };
type Edge = { from: string; to: string };

const CARD_WIDTH = 260;
const CARD_HEIGHT = 76;
const COLUMN_GAP = 24;
const ROW_GAP = 28;

/** Lay out parallel stages together using recorded dependencies. */
function positions(nodes: Node[], edges: Edge[]) {
  const ranks = new Map<string, number>();
  for (let pass = 0; pass < nodes.length; pass++) {
    for (const node of nodes) {
      const parents = edges.filter((edge) => edge.to === node.id);
      if (parents.every((edge) => ranks.has(edge.from))) {
        ranks.set(
          node.id,
          Math.max(0, ...parents.map((edge) => ranks.get(edge.from)! + 1)),
        );
      }
    }
  }
  const rows = new Map<number, Node[]>();
  for (const node of nodes) {
    const rank = ranks.get(node.id) ?? 0;
    rows.set(rank, [...(rows.get(rank) ?? []), node]);
  }
  const columns = Math.max(
    1,
    ...Array.from(rows.values(), (row) => row.length),
  );
  const width = columns * (CARD_WIDTH + COLUMN_GAP) - COLUMN_GAP;
  const height = rows.size * (CARD_HEIGHT + ROW_GAP) - ROW_GAP;
  const cards = nodes.map((node) => {
    const rank = ranks.get(node.id) ?? 0;
    const row = rows.get(rank)!;
    return {
      ...node,
      x:
        (width - row.length * (CARD_WIDTH + COLUMN_GAP) + COLUMN_GAP) / 2 +
        row.indexOf(node) * (CARD_WIDTH + COLUMN_GAP),
      y: rank * (CARD_HEIGHT + ROW_GAP),
    };
  });
  return { cards, width, height };
}

export default function ProcessingGraph({
  nodes,
  edges,
  selected,
  onSelect,
}: {
  nodes: Node[];
  edges: Edge[];
  selected?: string;
  onSelect: (id: string) => void;
}) {
  const marker = useId().replaceAll(":", "");
  const { cards, width, height } = positions(nodes, edges);
  if (!nodes.length) return <p>No stage evidence was recorded.</p>;
  return (
    <div
      className="overflow-x-auto rounded-box border border-base-300 bg-base-200/40 p-4"
      aria-label="Stage results"
    >
      <div className="relative mx-auto" style={{ width, height }}>
        <svg
          className="pointer-events-none absolute inset-0 h-full w-full text-base-content/35"
          aria-hidden="true"
        >
          <defs>
            <marker
              id={marker}
              viewBox="0 0 10 10"
              refX="9"
              refY="5"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M 0 0 L 10 5 L 0 10 z" fill="currentColor" />
            </marker>
          </defs>
          {edges.map((edge) => {
            const from = cards.find((node) => node.id === edge.from);
            const to = cards.find((node) => node.id === edge.to);
            if (!from || !to) return null;
            const x1 = from.x + CARD_WIDTH / 2;
            const y1 = from.y + CARD_HEIGHT;
            const x2 = to.x + CARD_WIDTH / 2;
            const y2 = to.y;
            const middle = (y1 + y2) / 2;
            return (
              <path
                key={`${edge.from}-${edge.to}`}
                data-from={edge.from}
                data-to={edge.to}
                d={`M ${x1} ${y1} C ${x1} ${middle}, ${x2} ${middle}, ${x2} ${y2}`}
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                markerEnd={`url(#${marker})`}
              />
            );
          })}
        </svg>
        {cards.map((node) => (
          <button
            key={node.id}
            type="button"
            aria-pressed={selected === node.id}
            aria-label={`${node.label} ${node.status}`}
            onClick={() => onSelect(node.id)}
            className={`absolute flex flex-col justify-between rounded-box border bg-base-100 p-3 text-left shadow-sm focus-visible:outline-2 focus-visible:outline-primary ${selected === node.id ? "border-primary ring-2 ring-primary/20" : "border-base-300"}`}
            style={{
              left: node.x,
              top: node.y,
              width: CARD_WIDTH,
              height: CARD_HEIGHT,
            }}
          >
            <strong className="text-sm">{node.label}</strong>
            <div className="flex items-center gap-2">
              <ProcessingStatus status={node.status} />
              <span
                className="truncate text-xs text-base-content/60"
                title={node.summary?.[0]}
              >
                {node.summary?.[0]}
              </span>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
