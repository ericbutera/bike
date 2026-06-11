"use client";

import { admin } from "@ericbutera/kaleido";
import Link from "next/link";
import { Suspense } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminAnalyticsPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Analytics">
          <div className="p-6">
            <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
              <h2 className="text-lg font-semibold">Analytics moved</h2>
              <p className="mt-2 max-w-2xl text-sm text-base-content/70">
                Admin maintenance actions now live with the background task
                tools, where queueing and task status can be managed together.
              </p>
              <div className="mt-5 flex flex-wrap gap-3">
                <Link href="/admin/manual-tasks" className="btn btn-primary">
                  Open task tools
                </Link>
                <Link href="/admin/metrics" className="btn btn-ghost">
                  View metrics
                </Link>
              </div>
            </section>
          </div>
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}
