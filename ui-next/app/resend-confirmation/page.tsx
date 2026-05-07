"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function ResendConfirmationPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.ResendConfirmation />
      </AuthRouter>
    </Suspense>
  );
}
