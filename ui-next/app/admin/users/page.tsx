"use client";

import { admin } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminUsersPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Users">
          <admin.Users />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}
