"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function ConfirmEmailPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.ConfirmEmail />
      </AuthRouter>
    </Suspense>
  );
}
