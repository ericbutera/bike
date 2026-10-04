import type { ReactNode } from "react";
import ReportsLayout from "../../../components/reports/ReportsLayout";

export default function TrainingReportsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ReportsLayout>{children}</ReportsLayout>;
}
