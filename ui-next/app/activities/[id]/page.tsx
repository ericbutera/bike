import ActivityDetailPanel from "../../../components/ActivityDetailPanel";
import Layout from "../../../components/Layout";

export default async function ActivityDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <Layout>
      <div className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-6 lg:px-8">
        <ActivityDetailPanel activityId={id} />
      </div>
    </Layout>
  );
}
