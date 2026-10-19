fromjson?
| select(.target == "sqlx::query")
| .fields as $query
| ((.spans // []) | map(select(.name == "request")) | first // {}) as $request
| ((.spans // []) | map(select(.name | startswith("activity."))) | first // {}) as $operation
| ($request["http.route"] // "") as $route
| select(($activities | not) or ($route | startswith("/api/activities")))
| (($query.elapsed_secs // 0) * 1000) as $elapsed_ms
| select($elapsed_ms >= $min_ms)
| (($query["db.statement"] // "") | gsub("^\\s+|\\s+$"; "")) as $statement
| "\(.timestamp) [\(.level)] \($elapsed_ms * 1000 | round / 1000) ms returned=\($query.rows_returned // 0) affected=\($query.rows_affected // 0)",
  "  \($request.method // "startup") \($route) \($operation.name // "")\(if $operation.activity_id then " activity_id=\($operation.activity_id)" else "" end) trace=\($request.trace_id // "-")",
  "  \(if $statement == "" then $query.summary else $statement end)",
  ""
