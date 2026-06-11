"use client";

import AdminTaskTools from "@/components/admin/AdminTaskTools";
import { admin } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminManualTasksPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Manual tasks">
          <div className="p-6">
            <AdminTaskTools />
          </div>
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}
