"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function ForgotPasswordPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.ForgotPassword />
      </AuthRouter>
    </Suspense>
  );
}
