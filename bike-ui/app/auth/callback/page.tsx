"use client";

import * as auth from "../../../lib/auth";
import { Suspense } from "react";

export default function AuthCallbackPage() {
  return (
    <Suspense>
      <auth.OAuthCallback />
    </Suspense>
  );
}
