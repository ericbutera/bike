"use client";

import { useEffect } from "react";
import { Suspense } from "react";
import { useRouter } from "next/navigation";
import Layout from "../../components/Layout";
import { config } from "../../lib/config";
import { authApiClient, OAuthSignIn } from "../../lib/auth";

export default function LoginPage() {
  return (
    <Suspense>
      {config.LOCAL_ADMIN_AUTO_LOGIN ? (
        <LocalAdminLogin />
      ) : (
        <Layout>
          <OAuthSignIn />
        </Layout>
      )}
    </Suspense>
  );
}

function LocalAdminLogin() {
  const router = useRouter();
  const { user, isLoading, isError } = authApiClient.useCurrentUser();

  useEffect(() => {
    if (user) {
      router.replace("/");
    }
  }, [router, user]);

  return (
    <Layout>
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <h1 className="card-title text-3xl">Local development access</h1>
          {isLoading ? (
            <p>Signing you in as the local admin…</p>
          ) : isError || !user ? (
            <>
              <p className="text-error">
                The local admin session could not be established. Confirm that
                the local Bike API is running and refresh this page.
              </p>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => window.location.reload()}
              >
                Retry
              </button>
            </>
          ) : (
            <p>Opening Bike…</p>
          )}
        </div>
      </section>
    </Layout>
  );
}
