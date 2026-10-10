import {
  apiBaseUrls,
  syntheticRequestHeaders,
} from "../../../../lib/serverApi";
import { headersWithTraceContext } from "../../../../lib/trace-context";

export const runtime = "nodejs";

function imageError(status: number) {
  return new Response(null, {
    status,
    headers: { "Cache-Control": "no-store" },
  });
}

export async function GET(
  request: Request,
  context: { params: Promise<{ variant: string; styleVersion: string }> },
) {
  const { variant, styleVersion } = await context.params;
  const syntheticHeaders = syntheticRequestHeaders(request.headers);
  if (syntheticHeaders === null) return imageError(403);
  const forwarded = new Headers({ Accept: "image/png", ...syntheticHeaders });
  for (const name of ["cookie", "authorization", "if-none-match"]) {
    const value = request.headers.get(name);
    if (value) forwarded.set(name, value);
  }
  const path = `/activity-map-images/${encodeURIComponent(variant)}/${encodeURIComponent(styleVersion)}${new URL(request.url).search}`;
  let imageResponse: Response | undefined;
  for (const baseUrl of apiBaseUrls()) {
    try {
      imageResponse = await fetch(`${baseUrl}${path}`, {
        headers: headersWithTraceContext(forwarded, request.headers),
        cache: "no-store",
        signal: AbortSignal.timeout(60000),
      });
      break;
    } catch {
      // A local Next process can also reach the configured localhost API.
    }
  }
  if (!imageResponse) return imageError(502);
  if (!imageResponse.ok && imageResponse.status !== 304) {
    return imageError(
      [400, 401, 403, 404].includes(imageResponse.status)
        ? imageResponse.status
        : 502,
    );
  }
  const headers = new Headers();
  for (const name of [
    "content-type",
    "cache-control",
    "vary",
    "etag",
    "x-map-cache",
  ]) {
    const value = imageResponse.headers.get(name);
    if (value) headers.set(name, value);
  }
  return new Response(
    imageResponse.status === 304 ? null : await imageResponse.arrayBuffer(),
    { status: imageResponse.status, headers },
  );
}
