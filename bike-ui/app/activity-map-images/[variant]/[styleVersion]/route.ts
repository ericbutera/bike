import { createHash } from "node:crypto";
import { ACTIVITY_MAP_STYLE_REVISION } from "../../../../lib/activityMapImages";
import { apiBaseUrls } from "../../../../lib/serverApi";
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
  const query = new URL(request.url).searchParams;
  const activityId = Number(query.get("activityId"));
  const theme = query.get("theme");
  const dpr = Number(query.get("dpr"));
  if (
    !Number.isSafeInteger(activityId) ||
    activityId <= 0 ||
    (variant !== "full" && variant !== "thumbnail") ||
    (theme !== "light" && theme !== "dark") ||
    (dpr !== 1 && dpr !== 2) ||
    styleVersion !== ACTIVITY_MAP_STYLE_REVISION
  )
    return imageError(400);

  const cookie = request.headers.get("cookie");
  const authorization = request.headers.get("authorization");
  let activityResponse: Response | null = null;
  for (const baseUrl of apiBaseUrls()) {
    try {
      activityResponse = await fetch(`${baseUrl}/activities/${activityId}`, {
        headers: headersWithTraceContext(
          {
            Accept: "application/json",
            ...(cookie ? { cookie } : {}),
            ...(authorization ? { authorization } : {}),
          },
          request.headers,
        ),
        cache: "no-store",
      });
      break;
    } catch {
      // A local Next process can reach localhost even when Docker's api name cannot resolve.
    }
  }
  if (!activityResponse) return imageError(502);
  if (!activityResponse.ok) {
    return imageError(
      [401, 403, 404].includes(activityResponse.status)
        ? activityResponse.status
        : 502,
    );
  }

  const activity = (await activityResponse.json()) as {
    route_points?: Array<{ latitude: number; longitude: number }> | null;
  };
  const points = (activity.route_points ?? [])
    .filter(
      (point) =>
        point &&
        Number.isFinite(point.latitude) &&
        Number.isFinite(point.longitude) &&
        Math.abs(point.latitude) <= 90 &&
        Math.abs(point.longitude) <= 180,
    )
    .map((point) => ({
      latitude: point.latitude,
      longitude: point.longitude,
    }));
  if (points.length < 2) return imageError(404);

  let imageResponse: Response;
  try {
    imageResponse = await fetch(
      `${process.env.MAP_RENDERER_URL ?? "http://bike-maps:3100"}/render`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(process.env.MAP_SERVICE_TOKEN
            ? { Authorization: `Bearer ${process.env.MAP_SERVICE_TOKEN}` }
            : {}),
        },
        body: JSON.stringify({
          points,
          theme,
          variant,
          dpr,
        }),
        cache: "no-store",
        signal: AbortSignal.timeout(60000),
      },
    );
  } catch {
    return imageError(502);
  }
  if (!imageResponse.ok) return imageError(502);

  const png = await imageResponse.arrayBuffer();
  const etag = `"${createHash("sha256").update(Buffer.from(png)).digest("hex")}"`;
  const responseHeaders = {
    "Content-Type": "image/png",
    "Cache-Control": "private, no-cache",
    Vary: "Cookie, Authorization",
    ETag: etag,
  };
  if (request.headers.get("if-none-match") === etag) {
    return new Response(null, { status: 304, headers: responseHeaders });
  }
  return new Response(png, { headers: responseHeaders });
}
