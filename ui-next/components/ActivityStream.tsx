"use client";

import { Pagination, auth, featureFlags } from "@ericbutera/kaleido";
import { faUpload } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { extractApiMessage } from "../lib/activityFormatting";
import { FLAG_ACTIVITY_LIST_FULL_MAPS } from "../lib/featureFlags";
import { useActivities } from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import ActivityStreamCard from "./activity-stream/ActivityStreamCard";
import AuthRequiredCard from "./AuthRequiredCard";

export default function ActivityStream() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const showFullRouteMaps = featureFlags.useFeatureFlag(
    FLAG_ACTIVITY_LIST_FULL_MAPS,
  );
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const currentUrlPage = parsePageParam(searchParams.get("page"));
  const [page, setPage] = useState(currentUrlPage);
  const perPage = 10;
  const activitiesQuery = useActivities({ enabled: !!user, page, perPage });

  useEffect(() => {
    setPage(currentUrlPage);
  }, [currentUrlPage]);

  const handlePageChange = (nextPage: number) => {
    const normalizedPage = Math.max(1, nextPage);
    const nextSearchParams = new URLSearchParams(searchParams.toString());

    setPage(normalizedPage);
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

  if (isLoadingUser) {
    return (
      <section className="rounded-box border border-base-300 bg-base-100 shadow-sm">
        <div className="flex items-center justify-center py-10">
          <span className="loading loading-spinner loading-md" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        eyebrow="Activity stream"
        title="Recent activity feed"
        description="Sign in to see your latest uploads normalized into a Garmin or Strava style stream."
        loginLabel="Sign in to view activities"
      />
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <h2 className="text-3xl font-semibold text-base-content">
          Recent activities
        </h2>
        {activitiesQuery.isFetching ? (
          <span className="loading loading-spinner loading-sm" />
        ) : null}

        <Link href="/upload" className="btn btn-ghost btn-sm">
          <FontAwesomeIcon icon={faUpload} className="h-8 w-8" />
          Upload Activity
        </Link>
      </div>

      {activitiesQuery.isError ? (
        <div className="alert alert-error">
          {extractApiMessage(activitiesQuery.error)}
        </div>
      ) : null}

      {!activitiesQuery.isError && activitiesQuery.data.length === 0 ? (
        <div className="alert bg-base-100 shadow-sm">
          <span>
            No activities yet. Upload a GPX, TCX, or FIT file below to seed your
            stream.
          </span>
        </div>
      ) : null}

      <div className="space-y-3">
        {activitiesQuery.data.map((activity) => (
          <ActivityStreamCard
            key={activity.id}
            activity={activity}
            unitSystem={unitSystem}
            showFullRouteMaps={showFullRouteMaps}
          />
        ))}
      </div>

      {(activitiesQuery.metadata?.total ?? 0) > perPage ? (
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
