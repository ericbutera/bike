"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function ResetPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.Reset />
      </AuthRouter>
    </Suspense>
  );
}
