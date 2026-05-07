"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function LoginPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.Login />
      </AuthRouter>
    </Suspense>
  );
}
