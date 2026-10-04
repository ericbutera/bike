import type { NamedStat } from "@/lib/queries";
import { StatItem } from "./LocalAdminLayout";

export default function BikeMetricsSection({
  stats,
  title = "Bike Activity",
}: {
  stats?: NamedStat[] | null;
  title?: string;
}) {
  if (!stats?.length) {
    return null;
  }

  return (
    <section className="mb-6">
      <h3 className="mb-3 text-lg font-semibold">{title}</h3>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
        {stats.map((stat) => (
          <StatItem
            key={stat.key}
            title={stat.label}
            value={stat.value.toLocaleString()}
            desc={stat.desc ?? undefined}
            error={stat.error ?? undefined}
          />
        ))}
      </div>
    </section>
  );
}
