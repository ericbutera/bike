import type { ReactNode } from "react";
import Layout from "../../components/Layout";
import RequireAuth from "../../components/RequireAuth";

export default function ImportsLayout({ children }: { children: ReactNode }) {
  return (
    <Layout>
      <div className="mx-auto grid max-w-6xl gap-6 px-4 py-8">
        <RequireAuth>{children}</RequireAuth>
      </div>
    </Layout>
  );
}
