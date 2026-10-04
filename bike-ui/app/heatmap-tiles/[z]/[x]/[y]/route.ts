import { apiBaseUrls } from "../../../../../lib/serverApi";
import { headersWithTraceContext } from "../../../../../lib/trace-context";

export const runtime = "nodejs";

export async function GET(
  request: Request,
  context: { params: Promise<{ z: string; x: string; y: string }> },
) {
  const { z, x, y } = await context.params;
  const query = new URL(request.url).searchParams;
  if (
    !/^\d{1,2}$/.test(z) ||
    !/^\d+$/.test(x) ||
    !/^\d+\.png$/.test(y) ||
    Number(z) > 18 ||
    Number(x) >= 2 ** Number(z) ||
    Number(y.slice(0, -4)) >= 2 ** Number(z) ||
    !/^\d+$/.test(query.get("revision") ?? "")
  ) {
    return new Response(null, {
      status: 400,
      headers: { "Cache-Control": "no-store" },
    });
  }
  const forwarded = new URLSearchParams();
  for (const key of ["sport", "from", "to", "revision"]) {
    const value = query.get(key);
    if (value !== null) forwarded.set(key, value);
  }
  const headers = headersWithTraceContext(
    { Accept: "image/png" },
    request.headers,
  );
  for (const key of ["cookie", "authorization", "if-none-match"]) {
    const value = request.headers.get(key);
    if (value) headers.set(key, value);
  }
  for (const baseUrl of apiBaseUrls()) {
    let upstream: Response;
    try {
      upstream = await fetch(
        `${baseUrl}/maps/heatmap/tiles/${z}/${x}/${y}?${forwarded}`,
        { headers, cache: "no-store", signal: request.signal },
      );
    } catch {
      if (request.signal.aborted)
        return new Response(null, {
          status: 499,
          headers: { "Cache-Control": "no-store" },
        });
      continue;
    }
    const responseHeaders = new Headers({
      "Cache-Control": "private, no-cache",
      Vary: "Cookie, Authorization",
    });
    for (const key of ["content-type", "etag", "retry-after"]) {
      const value = upstream.headers.get(key);
      if (value) responseHeaders.set(key, value);
    }
    if (!upstream.ok && upstream.status !== 304)
      responseHeaders.set("Cache-Control", "no-store");
    return new Response(upstream.status === 304 ? null : upstream.body, {
      status: upstream.status,
      headers: responseHeaders,
    });
  }
  return new Response(null, {
    status: 502,
    headers: { "Cache-Control": "no-store" },
  });
}
