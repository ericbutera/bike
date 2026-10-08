"use client";

import Link from "next/link";
import { useState } from "react";
import { useActivityImportHistory } from "../../lib/queries";
import { AppCard } from "../ui/Card";
import Pagination from "../ui/Pagination";
import { LoadingSpinner } from "../ui/QueryState";

export default function ImportHistory({
  archiveJobId,
}: {
  archiveJobId?: number;
}) {
  const [page, setPage] = useState(1);
  const [source, setSource] = useState("");
  const [status, setStatus] = useState("");
  const query = useActivityImportHistory({
    page,
    source: source || undefined,
    status: status || undefined,
    archive_job_id: archiveJobId,
  });
  return (
    <AppCard>
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold">Imports</h1>
        <Link href="/upload" className="btn btn-primary">
          Upload activity
        </Link>
      </div>
      <p className="mt-2 text-base-content/70">
        Follow every upload, archive entry, and Strava import. Open an import to
        inspect its stages or replay it.
      </p>
      <div className="my-6 flex flex-wrap gap-4">
        {archiveJobId ? (
          <p className="w-full">
            Archive job #{archiveJobId} ·{" "}
            <Link className="link" href="/imports">
              Show all imports
            </Link>
          </p>
        ) : null}
        <label>
          Source
          <select
            aria-label="Source"
            className="select select-bordered ml-2"
            value={source}
            onChange={(event) => {
              setSource(event.target.value);
              setPage(1);
            }}
          >
            <option value="">All sources</option>
            <option value="manual_upload">Upload</option>
            <option value="archive_url_import">Archive</option>
            <option value="strava_sync">Strava</option>
          </select>
        </label>
        <label>
          Outcome
          <select
            aria-label="Outcome"
            className="select select-bordered ml-2"
            value={status}
            onChange={(event) => {
              setStatus(event.target.value);
              setPage(1);
            }}
          >
            <option value="">All outcomes</option>
            {["processing", "processed", "duplicate", "failed", "canceled"].map(
              (value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ),
            )}
          </select>
        </label>
        {query.isFetching ? <LoadingSpinner size="sm" /> : null}
      </div>
      {query.error ? (
        <div className="alert alert-error">{query.error.message}</div>
      ) : null}
      <div className="overflow-x-auto">
        <table className="table">
          <thead>
            <tr>
              <th>Import</th>
              <th>Source</th>
              <th>Outcome / stage</th>
              <th>Received</th>
              <th>Activity</th>
            </tr>
          </thead>
          <tbody>
            {query.data?.items.map((item) => (
              <tr key={item.id}>
                <td>
                  <Link
                    className="link link-primary font-medium"
                    href={`/imports/${item.id}`}
                  >
                    {item.original_filename}
                  </Link>
                  <div className="text-xs">
                    {item.format.toUpperCase()} ·{" "}
                    {item.size_bytes.toLocaleString()} bytes
                  </div>
                </td>
                <td>{item.source}</td>
                <td>
                  {item.status}
                  <div className="text-xs text-base-content/65">
                    {item.processing_stage}
                  </div>
                  {item.processing_error ? (
                    <p className="max-w-sm text-sm text-error">
                      {item.processing_error}
                    </p>
                  ) : null}
                </td>
                <td className="whitespace-nowrap">
                  {new Date(item.created_at).toLocaleString()}
                </td>
                <td>
                  {item.activity_id ? (
                    <Link
                      className="link"
                      href={`/activities/${item.activity_id}`}
                    >
                      #{item.activity_id}
                    </Link>
                  ) : (
                    "No activity yet"
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {query.data?.total === 0 ? (
        <p className="py-8 text-center">No imports match these filters.</p>
      ) : null}
      {query.data ? (
        <Pagination
          className="mt-6"
          page={page}
          perPage={query.data.per_page}
          total={query.data.total}
          onPageChange={setPage}
        />
      ) : null}
    </AppCard>
  );
}
