import Layout from "../../../components/Layout";
import SegmentDetailPanel from "../../../components/SegmentDetailPanel";

export default async function SegmentDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-10 px-4 py-10 sm:px-6 lg:px-8">
        <SegmentDetailPanel segmentId={id} />
      </div>
    </Layout>
  );
}
