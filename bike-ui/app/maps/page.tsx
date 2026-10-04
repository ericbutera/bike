import { Suspense } from "react";
import HeatmapPanel from "../../components/HeatmapPanel";
import Layout from "../../components/Layout";
import RequireAuth from "../../components/RequireAuth";

export default function MapsPage() {
  return (
    <Layout mainClassName="h-[calc(100dvh-4rem)] overflow-hidden">
      <RequireAuth>
        <Suspense fallback={<p className="p-4">Loading maps…</p>}>
          <HeatmapPanel />
        </Suspense>
      </RequireAuth>
    </Layout>
  );
}
