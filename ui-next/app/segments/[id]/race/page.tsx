import RequireAuth from "../../../../components/RequireAuth";
import SegmentRaceViewer from "../../../../components/segment-detail/SegmentRaceViewer";
import {
  parseOptionalPositiveNumberParam,
  parseRacePlaybackSpeedParam,
  parseSelectedEffortIdsParam,
} from "../../../../lib/segmentDetail";

export default async function SegmentRacePage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { id } = await params;
  const resolvedSearchParams = await searchParams;

  return (
    <RequireAuth>
      <SegmentRaceViewer
        segmentId={id}
        initialSelectedEffortIds={parseSelectedEffortIdsParam(
          resolvedSearchParams.efforts,
        )}
        initialReferenceEffortId={parseOptionalPositiveNumberParam(
          resolvedSearchParams.ref,
        )}
        initialPlaybackSpeed={parseRacePlaybackSpeedParam(
          resolvedSearchParams.pace,
        )}
      />
    </RequireAuth>
  );
}
