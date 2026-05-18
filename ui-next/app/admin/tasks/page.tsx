"use client";

import { admin } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminTasksPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Dashboard">
          <admin.Tasks />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}
