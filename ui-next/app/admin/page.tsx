"use client";

import { admin } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function AdminPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Dashboard">
          <admin.Dashboard />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}
