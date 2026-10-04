import type { ReactNode } from "react";
import Layout from "../Layout";
import RequireAuth from "../RequireAuth";

export default function ReportsLayout({ children }: { children: ReactNode }) {
  return (
    <Layout>
      <RequireAuth>{children}</RequireAuth>
    </Layout>
  );
}
