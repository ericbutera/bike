import type { components } from "../../lib/openapi/react-query/api";
import {
  formatTaskDate,
  formatTaskDuration,
  millisecondsBetween,
} from "../../lib/taskTiming";

type Detail = components["schemas"]["TaskDetailResponse"];

export default function TaskProcessingHistory({ detail }: { detail: Detail }) {
  const now = new Date().toISOString();
  const attempts = detail.attempt_history ?? [];
  return (
    <section className="mt-6 space-y-4" aria-label="Processing history">
      <h4 className="font-semibold">Processing history</h4>
      {attempts.length === 0 ? (
        <p className="text-sm text-base-content/70">
          No execution attempts have been recorded.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table className="table table-sm">
            <caption className="text-left text-sm font-semibold">
              Execution attempts
            </caption>
            <thead>
              <tr>
                <th>Attempt</th>
                <th>Outcome</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Last heartbeat</th>
                <th>Error / trace</th>
              </tr>
            </thead>
            <tbody>
              {attempts.map((attempt) => (
                <tr key={attempt.attempt}>
                  <td>{attempt.attempt}</td>
                  <td>{attempt.outcome}</td>
                  <td>{formatTaskDate(attempt.started_at)}</td>
                  <td>
                    {formatTaskDuration(
                      millisecondsBetween(
                        attempt.started_at,
                        attempt.finished_at ?? now,
                      ),
                    )}
                  </td>
                  <td>{formatTaskDate(attempt.heartbeat_at)}</td>
                  <td className="max-w-xs break-words">
                    {attempt.error ? <p>{attempt.error}</p> : null}
                    {attempt.trace_id ? (
                      <p className="font-mono text-xs">
                        Trace: {attempt.trace_id}
                      </p>
                    ) : null}
                    {attempt.span_id ? (
                      <p className="font-mono text-xs">
                        Span: {attempt.span_id}
                      </p>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
