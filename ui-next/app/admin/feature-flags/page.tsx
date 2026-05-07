"use client";

import { admin } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function FeatureFlagsPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Feature Flags">
          <admin.FeatureFlags />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}
