import { formatActivityTimestamp } from "@/lib/activityFormatting";
import type { IntegrationEvent } from "@/lib/queries";

export default function StravaEventHistory({
  events,
}: {
  events: IntegrationEvent[];
}) {
  return (
    <table className="table table-zebra w-full">
      <thead>
        <tr>
          <th>When</th>
          <th>Request</th>
        </tr>
      </thead>
      <tbody>
        {events.map((event) => (
          <tr key={event.id}>
            <td>{formatActivityTimestamp(event.created_at)}</td>
            <td>{event.message}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
