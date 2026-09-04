import Layout from "../../../components/Layout";
import RequireAuth from "../../../components/RequireAuth";
import SegmentEffortAnalysisReport from "../../../components/SegmentEffortAnalysisReport";

export default function SegmentAnalysisPage() {
  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-8 px-4 py-8 sm:px-6 lg:px-8">
        <RequireAuth>
          <SegmentEffortAnalysisReport />
        </RequireAuth>
      </div>
    </Layout>
  );
}
