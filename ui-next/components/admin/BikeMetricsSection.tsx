import type { NamedStat } from "@/lib/queries";
import { admin } from "@ericbutera/kaleido";

export default function BikeMetricsSection({
  stats,
}: {
  stats?: NamedStat[] | null;
}) {
  if (!stats?.length) {
    return null;
  }

  return (
    <section className="mb-6">
      <h3 className="mb-3 text-lg font-semibold">Bike Activity</h3>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
        {stats.map((stat) => (
          <admin.StatItem
            key={stat.key}
            title={stat.label}
            value={stat.value.toLocaleString()}
            desc={stat.desc}
            error={stat.error}
          />
        ))}
      </div>
    </section>
  );
}
