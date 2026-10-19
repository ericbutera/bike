"use client";

import { faUpload } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import {
  FLAG_ACTIVITY_LIST_FULL_MAPS,
  FLAG_ENHANCED_MAPS,
} from "../lib/featureFlags";
import { useFeatureFlag } from "../lib/featureFlags";
import { useActivities } from "../lib/queries";
import { parseActivitySport } from "../lib/activitySports";
import { useUnitPreferences } from "../lib/unitPreferences";
import { useKeyedState } from "../lib/useKeyedState";
import ActivityStreamCard from "./activity-stream/ActivityStreamCard";
import ActivitySportFilter from "./activity-stream/ActivitySportFilter";
import { ErrorCard, LoadingCard, LoadingSpinner } from "./ui/QueryState";
import Pagination from "./ui/Pagination";

export default function ActivityStream() {
  const { unitSystem } = useUnitPreferences();
  const showFullRouteMaps = useFeatureFlag(FLAG_ACTIVITY_LIST_FULL_MAPS);
  const showEnhancedMaps = useFeatureFlag(FLAG_ENHANCED_MAPS);
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const search = searchParams.toString();
  const [selection, setSelection] = useKeyedState(search, {
    page: parsePageParam(searchParams.get("page")),
    sport: parseActivitySport(searchParams.get("sport")),
  });
  const { page, sport } = selection;
  const perPage = 10;
  const activitiesQuery = useActivities({
    page,
    perPage,
    ...(sport ? { sport } : {}),
  });

  const handlePageChange = (nextPage: number) => {
    const normalizedPage = Math.max(1, nextPage);
    setSelection((current) => ({ ...current, page: normalizedPage }));
    const nextSearchParams = new URLSearchParams(searchParams.toString());

    if (normalizedPage === 1) {
      nextSearchParams.delete("page");
    } else {
      nextSearchParams.set("page", String(normalizedPage));
    }

    const query = nextSearchParams.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, {
      scroll: false,
    });
  };

  const handleSportChange = (value: string) => {
    const nextSport = parseActivitySport(value);
    setSelection({ page: 1, sport: nextSport });
    const nextSearchParams = new URLSearchParams(searchParams.toString());

    nextSearchParams.delete("page");
    if (nextSport) {
      nextSearchParams.set("sport", nextSport);
    } else {
      nextSearchParams.delete("sport");
    }

    const query = nextSearchParams.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, {
      scroll: false,
    });
  };

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <h2 className="text-3xl font-semibold text-base-content">
          Recent activities
        </h2>
        <div className="flex items-center gap-3">
          {activitiesQuery.isFetching ? <LoadingSpinner size="sm" /> : null}
          <div className="flex flex-wrap items-center gap-2">
            <ActivitySportFilter value={sport} onChange={handleSportChange} />
            <Link
              href="/upload"
              className="btn btn-sm btn-square"
              aria-label="Upload Activity"
              title="Upload Activity"
            >
              <FontAwesomeIcon
                icon={faUpload}
                className="h-4 w-4"
                aria-hidden="true"
              />
            </Link>
          </div>
        </div>
      </div>

      {activitiesQuery.isLoading ? <LoadingCard /> : null}

      {activitiesQuery.isError ? (
        <ErrorCard fallback="Unable to load activities." />
      ) : null}

      {!activitiesQuery.isLoading &&
      !activitiesQuery.isError &&
      activitiesQuery.data?.length === 0 ? (
        <div className="alert bg-base-100 shadow-sm">
          <span>
            No activities yet. Upload a GPX, TCX, or FIT file below to seed your
            stream.
          </span>
        </div>
      ) : null}

      <div className="space-y-3">
        {!activitiesQuery.isLoading && !activitiesQuery.isError
          ? activitiesQuery.data?.map((activity) => (
              <ActivityStreamCard
                key={activity.id}
                activity={activity}
                unitSystem={unitSystem}
                showFullRouteMaps={showFullRouteMaps}
                showEnhancedMaps={showEnhancedMaps}
              />
            ))
          : null}
      </div>

      {!activitiesQuery.isLoading &&
      !activitiesQuery.isError &&
      activitiesQuery.metadata &&
      activitiesQuery.metadata.total > perPage ? (
        <Pagination
          page={activitiesQuery.metadata.page}
          perPage={activitiesQuery.metadata.per_page}
          total={activitiesQuery.metadata.total}
          onPageChange={handlePageChange}
        />
      ) : null}
    </section>
  );
}

function parsePageParam(rawPage: string | null): number {
  const parsedPage = Number(rawPage);

  if (Number.isFinite(parsedPage) && parsedPage >= 1) {
    return Math.floor(parsedPage);
  }

  return 1;
}
