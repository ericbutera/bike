"use client";

import { auth } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../components/AuthRouter";

export default function SignUpPage() {
  return (
    <Suspense>
      <AuthRouter>
        <auth.SignUp />
      </AuthRouter>
    </Suspense>
  );
}
