import type { ReactNode } from "react";
import RequireAuth from "../RequireAuth";

export default function ReportsLayout({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen bg-base-200">
      <RequireAuth>{children}</RequireAuth>
    </div>
  );
}
