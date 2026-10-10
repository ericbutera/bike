import type { ReactNode } from "react";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

export default function ImportsLayout({ children }: { children: ReactNode }) {
  return <LocalAdminLayout title="Import history">{children}</LocalAdminLayout>;
}
