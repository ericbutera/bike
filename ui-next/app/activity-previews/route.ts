import {
  buildRoutePreviewGeometry,
  parseRoutePreviewCoordinates,
  resolveRoutePreviewVariant,
  ROUTE_PREVIEW_STYLE_VERSION,
} from "../../lib/routePreview";
import { getServerConfig } from "../../lib/config";

export const runtime = "nodejs";

type RoutePreviewCoordinate = {
  latitude: number;
  longitude: number;
};

function resolveActivityApiBaseUrls() {
  const explicitInternalApiUrl = process.env.INTERNAL_API_URL?.trim();
  const publicApiUrl = getServerConfig().API_URL.replace(/\/$/, "");

  if (explicitInternalApiUrl) {
    return [explicitInternalApiUrl.replace(/\/$/, ""), publicApiUrl];
  }

  const candidates = [publicApiUrl];

  try {
    const parsedUrl = new URL(publicApiUrl);

    if (
      parsedUrl.hostname === "localhost" ||
      parsedUrl.hostname === "127.0.0.1"
    ) {
      parsedUrl.hostname = "api";
      candidates.unshift(parsedUrl.toString().replace(/\/$/, ""));
    }
  } catch {
    return candidates;
  }

  return Array.from(new Set(candidates));
}

function buildContourPath(
  width: number,
  height: number,
  startY: number,
  amplitude: number,
  tilt: number,
) {
  return [
    `M ${-width * 0.08} ${startY.toFixed(1)}`,
    `C ${(width * 0.14).toFixed(1)} ${(startY - amplitude * 0.85).toFixed(1)},`,
    `${(width * 0.34).toFixed(1)} ${(startY + amplitude * 0.55).toFixed(1)},`,
    `${(width * 0.54).toFixed(1)} ${(startY - amplitude * 0.35).toFixed(1)}`,
    `S ${(width * 0.88).toFixed(1)} ${(startY + amplitude * tilt).toFixed(1)},`,
    `${(width * 1.08).toFixed(1)} ${(startY - amplitude * 0.45).toFixed(1)}`,
  ].join(" ");
}

function buildPreviewSvg(
  routePoints: RoutePreviewCoordinate[],
  variant: ReturnType<typeof resolveRoutePreviewVariant>,
) {
  const geometry = buildRoutePreviewGeometry(routePoints, variant);
  const fallbackWidth = variant === "thumbnail" ? 288 : 1000;
  const fallbackHeight = variant === "thumbnail" ? 192 : 300;
  const width = geometry?.width ?? fallbackWidth;
  const height = geometry?.height ?? fallbackHeight;
  const contourOffsets = [0.16, 0.33, 0.52, 0.72, 0.9];
  const routeCasingWidth = variant === "thumbnail" ? 12 : 14;
  const routeCoreWidth = variant === "thumbnail" ? 7 : 9;
  const startRadius = variant === "thumbnail" ? 8 : 10;
  const startStrokeWidth = variant === "thumbnail" ? 3 : 4;
  const endRadius = variant === "thumbnail" ? 8.5 : 10.5;
  const endStrokeWidth = variant === "thumbnail" ? 3.5 : 4.5;

  const contourPaths = contourOffsets
    .map((offset, index) => {
      const path = buildContourPath(
        width,
        height,
        height * offset,
        height * (0.08 + index * 0.012),
        index % 2 === 0 ? 0.4 : 0.8,
      );

      return `<path d="${path}" fill="none" stroke="#10212d" stroke-opacity="${0.045 + index * 0.012}" stroke-width="${1.4 + index * 0.2}" stroke-linecap="round" />`;
    })
    .join("");

  const routeMarkup = geometry
    ? `
      <path d="${geometry.pathData}" fill="none" stroke="#0f2631" stroke-width="${routeCasingWidth}" stroke-linecap="round" stroke-linejoin="round" stroke-opacity="0.78" />
      <path d="${geometry.pathData}" fill="none" stroke="#16b8a5" stroke-width="${routeCoreWidth}" stroke-linecap="round" stroke-linejoin="round" />
      <circle cx="${geometry.startPoint.x.toFixed(1)}" cy="${geometry.startPoint.y.toFixed(1)}" r="${startRadius}" fill="#0f172a" stroke="#f8fafc" stroke-width="${startStrokeWidth}" />
      <circle cx="${geometry.endPoint.x.toFixed(1)}" cy="${geometry.endPoint.y.toFixed(1)}" r="${endRadius}" fill="#f8fafc" stroke="#0f172a" stroke-width="${endStrokeWidth}" />
    `
    : `
      <rect x="${(width * 0.14).toFixed(1)}" y="${(height * 0.32).toFixed(1)}" width="${(width * 0.72).toFixed(1)}" height="${(height * 0.36).toFixed(1)}" rx="${variant === "thumbnail" ? 20 : 36}" fill="#fffaf0" fill-opacity="0.75" stroke="#d2d8d2" stroke-width="2" />
      <path d="M ${(width * 0.22).toFixed(1)} ${(height * 0.42).toFixed(1)} L ${(width * 0.78).toFixed(1)} ${(height * 0.58).toFixed(1)}" stroke="#cad3cd" stroke-width="${variant === "thumbnail" ? 6 : 14}" stroke-linecap="round" stroke-dasharray="${variant === "thumbnail" ? "0 18" : "0 34"}" />
    `;

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" fill="none">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#f8f2e8" />
      <stop offset="52%" stop-color="#edf2ea" />
      <stop offset="100%" stop-color="#dfe9e4" />
    </linearGradient>
    <linearGradient id="glow" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#ffffff" stop-opacity="0.85" />
      <stop offset="100%" stop-color="#ffffff" stop-opacity="0" />
    </linearGradient>
    <pattern id="grid" width="56" height="56" patternUnits="userSpaceOnUse">
      <path d="M 56 0 L 0 0 0 56" fill="none" stroke="#10212d" stroke-opacity="0.04" stroke-width="1" />
    </pattern>
  </defs>
  <rect width="${width}" height="${height}" rx="${variant === "thumbnail" ? 24 : 42}" fill="url(#bg)" />
  <rect width="${width}" height="${height}" rx="${variant === "thumbnail" ? 24 : 42}" fill="url(#grid)" />
  ${contourPaths}
  <path d="M ${(-width * 0.02).toFixed(1)} ${(height * 0.18).toFixed(1)} C ${(width * 0.18).toFixed(1)} ${(height * 0.08).toFixed(1)}, ${(width * 0.44).toFixed(1)} ${(height * 0.3).toFixed(1)}, ${(width * 0.7).toFixed(1)} ${(height * 0.18).toFixed(1)} S ${(width * 0.95).toFixed(1)} ${(height * 0.26).toFixed(1)}, ${(width * 1.04).toFixed(1)} ${(height * 0.1).toFixed(1)}" fill="none" stroke="#6f8f86" stroke-opacity="0.11" stroke-width="${variant === "thumbnail" ? 6 : 6}" stroke-linecap="round" stroke-dasharray="${variant === "thumbnail" ? "1 16" : "1 24"}" />
  <path d="M ${(-width * 0.04).toFixed(1)} ${(height * 0.82).toFixed(1)} C ${(width * 0.2).toFixed(1)} ${(height * 0.68).toFixed(1)}, ${(width * 0.42).toFixed(1)} ${(height * 0.92).toFixed(1)}, ${(width * 0.66).toFixed(1)} ${(height * 0.74).toFixed(1)} S ${(width * 0.92).toFixed(1)} ${(height * 0.86).toFixed(1)}, ${(width * 1.06).toFixed(1)} ${(height * 0.7).toFixed(1)}" fill="none" stroke="#385a66" stroke-opacity="0.085" stroke-width="${variant === "thumbnail" ? 5 : 5}" stroke-linecap="round" stroke-dasharray="${variant === "thumbnail" ? "20 16" : "28 20"}" />
  <rect width="${width}" height="${height}" rx="${variant === "thumbnail" ? 24 : 42}" fill="url(#glow)" />
  ${routeMarkup}
</svg>`;
}

async function loadActivityRoutePoints(
  request: Request,
  activityId: number,
): Promise<RoutePreviewCoordinate[]> {
  const cookie = request.headers.get("cookie");
  const authorization = request.headers.get("authorization");

  for (const apiBaseUrl of resolveActivityApiBaseUrls()) {
    try {
      const response = await fetch(`${apiBaseUrl}/activities/${activityId}`, {
        headers: {
          Accept: "application/json",
          ...(cookie ? { cookie } : {}),
          ...(authorization ? { authorization } : {}),
        },
        cache: "no-store",
      });

      if (!response.ok) {
        continue;
      }

      const payload = (await response.json()) as {
        route_points?: Array<Partial<RoutePreviewCoordinate>> | null;
      };

      return (payload.route_points ?? []).flatMap((point) => {
        if (
          !Number.isFinite(point.latitude) ||
          !Number.isFinite(point.longitude)
        ) {
          return [];
        }

        return [
          {
            latitude: Number(point.latitude),
            longitude: Number(point.longitude),
          },
        ];
      });
    } catch {
      continue;
    }
  }

  return [];
}

export async function handlePreviewRequest(
  request: Request,
  forcedVariant?: ReturnType<typeof resolveRoutePreviewVariant>,
) {
  const { searchParams } = new URL(request.url);
  const requestedVersion = searchParams.get("v");
  const variant =
    forcedVariant ?? resolveRoutePreviewVariant(searchParams.get("variant"));
  const requestedActivityId = Number(searchParams.get("activityId"));
  const usesActivityGeometry =
    Number.isFinite(requestedActivityId) && requestedActivityId > 0;
  const routePoints = usesActivityGeometry
    ? await loadActivityRoutePoints(request, requestedActivityId)
    : parseRoutePreviewCoordinates(searchParams.get("points"));
  const svg = buildPreviewSvg(routePoints, variant);

  const headers = new Headers({
    "Content-Type": "image/svg+xml",
    "Cache-Control": usesActivityGeometry
      ? "private, max-age=31536000, immutable"
      : "public, max-age=31536000, immutable",
  });

  if (usesActivityGeometry) {
    headers.set("Vary", "Cookie");
  }

  if (requestedVersion !== ROUTE_PREVIEW_STYLE_VERSION) {
    headers.set("X-Route-Preview-Version", ROUTE_PREVIEW_STYLE_VERSION);
  }

  return new Response(svg, { headers });
}

export async function GET(request: Request) {
  return handlePreviewRequest(request);
}
