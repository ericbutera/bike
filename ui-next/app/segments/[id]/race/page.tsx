import SegmentRaceViewer from "../../../../components/segment-detail/SegmentRaceViewer";
import type { PlaybackPace } from "../../../../lib/segmentDetail";

function firstValue(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}

function parseSelectedEffortIds(value: string | string[] | undefined) {
  const raw = firstValue(value);

  if (!raw) {
    return [] as number[];
  }

  return raw
    .split(",")
    .map((entry) => Number(entry.trim()))
    .filter((entry) => Number.isFinite(entry) && entry > 0);
}

function parseOptionalNumber(value: string | string[] | undefined) {
  const numericValue = Number(firstValue(value));

  return Number.isFinite(numericValue) && numericValue > 0
    ? numericValue
    : null;
}

function parsePlaybackPace(
  value: string | string[] | undefined,
): PlaybackPace | undefined {
  const nextValue = firstValue(value);

  return nextValue === "detail" || nextValue === "auto" || nextValue === "fast"
    ? nextValue
    : undefined;
}

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
    <SegmentRaceViewer
      segmentId={id}
      initialSelectedEffortIds={parseSelectedEffortIds(
        resolvedSearchParams.efforts,
      )}
      initialReferenceEffortId={parseOptionalNumber(resolvedSearchParams.ref)}
      initialPlaybackPace={parsePlaybackPace(resolvedSearchParams.pace)}
    />
  );
}
