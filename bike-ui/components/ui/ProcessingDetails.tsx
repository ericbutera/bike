import type { ReactNode } from "react";
import StatusBadge from "./StatusBadge";

export function ProcessingStatus({ status }: { status: string }) {
  return <StatusBadge status={status} label={status.replaceAll("_", " ")} />;
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
