import type { BikeTheme } from "./useBikeTheme";
import type { RoutePreviewVariant } from "./routePreview";

export const ACTIVITY_MAP_STYLE_REVISION = "1";

export function activityMapImageUrl({
  activityId,
  variant,
  theme,
  dpr,
}: {
  activityId: number;
  variant: RoutePreviewVariant;
  theme: BikeTheme;
  dpr: 1 | 2;
}) {
  return `/activity-map-images/${variant}/${ACTIVITY_MAP_STYLE_REVISION}?activityId=${activityId}&theme=${theme}&dpr=${dpr}`;
}
