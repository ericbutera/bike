import type { ReactNode } from "react";

export function ProcessingStatus({ status }: { status: string }) {
  const colors: Record<string, string> = {
    completed: "badge-success",
    processed: "badge-success",
    available: "badge-success",
    published: "badge-success",
    failed: "badge-error",
    running: "badge-info",
    processing: "badge-info",
    reused: "badge-info",
    queued: "badge-warning",
    retrying: "badge-warning",
  };
  const color = colors[status];
  return (
    <span className={`badge badge-outline badge-sm ${color ?? ""}`}>
      {status.replaceAll("_", " ")}
    </span>
  );
}

export function ProcessingFacts({
  facts,
}: {
  facts: { label: string; value: ReactNode }[];
}) {
  return (
    <dl className="grid gap-4 sm:grid-cols-2">
      {facts.map(({ label, value }) => (
        <div key={label} className="min-w-0">
          <dt className="text-xs font-medium text-base-content/60">{label}</dt>
          <dd className="mt-1 break-words text-sm">
            {value ?? "Not recorded"}
          </dd>
        </div>
      ))}
    </dl>
  );
}
