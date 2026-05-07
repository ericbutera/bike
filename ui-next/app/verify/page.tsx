"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function VerifyPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.Verify />
      </AuthRouter>
    </Suspense>
  );
}
