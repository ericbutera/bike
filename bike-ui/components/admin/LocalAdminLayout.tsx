"use client";

import type { ReactNode } from "react";
import Layout from "../Layout";
import RequireAdmin from "../RequireAdmin";
import Nav from "./Nav";

export function LocalAdminLayout({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <Layout>
      <RequireAdmin>
        <div className="container mx-auto p-6">
          <h1 className="mb-4 text-2xl font-bold">{title}</h1>
          <div className="grid grid-cols-12 gap-6">
            <aside className="col-span-12 md:col-span-3">
              <div className="card bg-base-100 p-4">
                <Nav />
              </div>
            </aside>
            <main className="col-span-12 min-w-0 md:col-span-9">
              {children}
            </main>
          </div>
        </div>
      </RequireAdmin>
    </Layout>
  );
}

export function StatItem({
  title,
  value,
  desc,
  error,
}: {
  title: string;
  value: ReactNode;
  desc?: string;
  error?: string;
}) {
  return (
    <div className="stat bg-base-100 shadow">
      {error ? (
        <div className="stat-figure">
          <div className="tooltip tooltip-left" data-tip={error}>
            <span
              className="text-warning text-2xl cursor-help"
              aria-label={`Error: ${error}`}
            >
              ⚠
            </span>
          </div>
        </div>
      ) : null}
      <div className="stat-title">{title}</div>
      <div className="stat-value">{error ? "—" : value}</div>
      {desc ? <div className="stat-desc">{desc}</div> : null}
    </div>
  );
}
