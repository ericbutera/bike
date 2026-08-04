import Layout from "../../../components/Layout";
import RequireAuth from "../../../components/RequireAuth";
import SegmentDetailPanel from "../../../components/SegmentDetailPanel";
import {
  parseOptionalPositiveNumberParam,
  parseSelectedEffortIdsParam,
} from "../../../lib/segmentDetail";

export default async function SegmentDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { id } = await params;
  const resolvedSearchParams = await searchParams;

  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-10 px-4 py-10 sm:px-6 lg:px-8">
        <RequireAuth>
          <SegmentDetailPanel
            segmentId={id}
            initialSelectedEffortIds={parseSelectedEffortIdsParam(
              resolvedSearchParams.efforts,
            )}
            initialReferenceEffortId={parseOptionalPositiveNumberParam(
              resolvedSearchParams.ref,
            )}
          />
        </RequireAuth>
      </div>
    </Layout>
  );
}
