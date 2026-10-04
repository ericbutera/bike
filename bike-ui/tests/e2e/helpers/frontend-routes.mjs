import { activityId, raceEffortIds, segmentId } from "./targets.mjs";

// This is the shared browser route inventory. Keep route names stable: they
// are used in the artifact names and in the shared parity evidence.
export const frontendRoutes = [
  { name: "activity-list", path: "/", surface: "activities" },
  {
    name: "activity-detail",
    path: `/activities/${activityId}`,
    surface: "activities",
  },
  { name: "segments", path: "/segments", surface: "segments" },
  {
    name: "segment-detail",
    path: `/segments/${segmentId}`,
    surface: "segments",
  },
  {
    name: "race-viewer",
    path: `/segments/${segmentId}/race?efforts=${raceEffortIds}`,
    surface: "segments",
  },
  {
    name: "segment-analysis",
    path: `/segments/analysis?segment_id=${segmentId}`,
    surface: "segments",
  },
  {
    name: "segment-builder",
    path: `/segments/builder?activityId=${activityId}`,
    surface: "segments",
  },
  {
    name: "segment-progress",
    path: `/segments/progress?segment_id=${segmentId}`,
    surface: "segments",
  },
  { name: "xc-training", path: "/xc", surface: "training" },
  { name: "dh-training", path: "/dh", surface: "training" },
  { name: "fitness", path: "/fitness", surface: "training" },
  // Reports persist their default range in the URL during hydration.
  {
    name: "training-reports",
    path: "/training/reports?range=week",
    surface: "reports",
  },
  { name: "account", path: "/account", surface: "account" },
  { name: "upload-import", path: "/upload", surface: "imports" },
  { name: "admin-dashboard", path: "/admin", surface: "admin" },
  { name: "admin-activities", path: "/admin/activities", surface: "admin" },
  { name: "admin-analytics", path: "/admin/analytics", surface: "admin" },
  {
    name: "admin-feature-flags",
    path: "/admin/feature-flags",
    surface: "admin",
  },
  {
    name: "admin-integrations",
    path: "/admin/integrations?provider=strava",
    surface: "admin",
  },
  { name: "admin-manual-tasks", path: "/admin/manual-tasks", surface: "admin" },
  { name: "admin-metrics", path: "/admin/metrics", surface: "admin" },
  { name: "admin-tasks", path: "/admin/tasks", surface: "admin" },
  { name: "admin-users", path: "/admin/users", surface: "admin" },
];

export function selectedFrontendRoutes() {
  const requestedRoute = process.env.PLAYWRIGHT_ROUTE?.trim();
  if (!requestedRoute) return frontendRoutes;

  const requestedNames = requestedRoute.split(",").map((name) => name.trim());
  const routes = requestedNames.map((name) =>
    frontendRoutes.find((route) => route.name === name),
  );
  if (routes.some((route) => !route)) {
    throw new Error(
      `Unknown PLAYWRIGHT_ROUTE=${requestedRoute}; expected one of ${frontendRoutes
        .map(({ name }) => name)
        .join(", ")}`,
    );
  }

  return routes;
}

export const protectedFrontendRoutes = frontendRoutes.filter(
  ({ name }) => name !== "activity-list",
);
