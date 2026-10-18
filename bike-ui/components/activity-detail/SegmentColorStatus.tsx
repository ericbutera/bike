export default function SegmentColorStatus({ color }: { color: string }) {
  return (
    <span
      aria-hidden="true"
      className="status status-lg shrink-0 rounded-full bg-none shadow-none"
      style={{ backgroundColor: color }}
    />
  );
}
