export default function DetailMetric({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="stats border border-base-300 bg-base-200 shadow-sm">
      <div className="stat px-4 py-4">
        <div className="stat-title">{label}</div>
        <div className="stat-value text-lg sm:text-xl">{value}</div>
      </div>
    </div>
  );
}
