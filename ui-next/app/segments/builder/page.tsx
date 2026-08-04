import Layout from "../../../components/Layout";
import RequireAuth from "../../../components/RequireAuth";
import SegmentBuilderPage from "../../../components/SegmentBuilderPage";

export default function SegmentBuilderRoute() {
  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-8 px-4 py-8 sm:px-6 lg:px-8">
        <RequireAuth>
          <SegmentBuilderPage />
        </RequireAuth>
      </div>
    </Layout>
  );
}
