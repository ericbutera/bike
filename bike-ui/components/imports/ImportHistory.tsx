"use client";

import Link from "next/link";
import { useState } from "react";
import { useActivityImportHistory } from "../../lib/queries";
import { DataGrid, ListCard, type Column } from "../ui/DataGrid";
import Pagination from "../ui/Pagination";
import RefreshButton from "../ui/RefreshButton";
import StatusBadge from "../ui/StatusBadge";

type ImportItem = NonNullable<
  ReturnType<typeof useActivityImportHistory>["data"]
>["items"][number];

const columns: Column<ImportItem>[] = [
  {
    key: "id",
    header: "ID",
    className: "whitespace-nowrap",
    render: (item) => item.id,
  },
  {
    key: "filename",
    header: "Import",
    className: "max-w-xs truncate",
    render: (item) => (
      <Link
        title={item.original_filename}
        className="link link-primary font-medium"
        href={`/admin/imports/${item.id}`}
      >
        {item.original_filename}
      </Link>
    ),
  },
  {
    key: "source",
    header: "Source",
    className: "whitespace-nowrap",
    render: (item) => item.source,
  },
  {
    key: "format",
    header: "Format",
    render: (item) => (
      <span title={`${item.size_bytes.toLocaleString()} bytes`}>
        {item.format.toUpperCase()}
      </span>
    ),
  },
  {
    key: "status",
    header: "Outcome",
    render: (item) => <StatusBadge status={item.status} />,
  },
  {
    key: "stage",
    header: "Stage",
    className: "whitespace-nowrap",
    render: (item) => <StatusBadge status={item.processing_stage} />,
  },
  {
    key: "error",
    header: "Error",
    className: "max-w-xs truncate",
    render: (item) => (
      <span
        className={
          item.processing_error ? "text-error" : "text-base-content/50"
        }
        title={item.processing_error ?? undefined}
      >
        {item.processing_error || "—"}
      </span>
    ),
  },
  {
    key: "activity",
    header: "Activity",
    className: "whitespace-nowrap",
    render: (item) =>
      item.activity_id ? (
        <Link className="link" href={`/activities/${item.activity_id}`}>
          #{item.activity_id}
        </Link>
      ) : (
        "No activity yet"
      ),
  },
  {
    key: "received",
    header: "Received",
    className: "whitespace-nowrap",
    render: (item) => new Date(item.created_at).toLocaleString(),
  },
];

export default function ImportHistory({
  archiveJobId,
}: {
  archiveJobId?: number;
}) {
  const [page, setPage] = useState(1);
  const [source, setSource] = useState("");
  const [status, setStatus] = useState("");
  const [userId, setUserId] = useState("");
  const validUserId =
    !userId || (Number.isInteger(Number(userId)) && Number(userId) > 0);
  const query = useActivityImportHistory(
    {
      page,
      source: source || undefined,
      status: status || undefined,
      archive_job_id: archiveJobId,
      user_id: userId ? Number(userId) : undefined,
    },
    { enabled: validUserId },
  );

  return (
    <ListCard
      title="Import history"
      filterActions={
        <RefreshButton
          disabled={query.isFetching || !validUserId}
          onClick={() => void query.refetch()}
        />
      }
      onSearch={() => {
        if (validUserId) void query.refetch();
      }}
      onClear={() => {
        setSource("");
        setStatus("");
        setUserId("");
        setPage(1);
      }}
      filters={
        <>
          <input
            aria-label="User ID"
            aria-invalid={!validUserId}
            title="User ID"
            className="input input-sm input-bordered w-40"
            type="number"
            min="1"
            step="1"
            placeholder="Your account"
            value={userId}
            onChange={(event) => {
              setUserId(event.target.value);
              setPage(1);
            }}
          />
          <select
            aria-label="Source"
            className="select select-sm select-bordered w-40"
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
          <select
            aria-label="Outcome"
            className="select select-sm select-bordered w-40"
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
        </>
      }
    >
      {archiveJobId ? (
        <p className="text-sm">
          Archive job #{archiveJobId} ·{" "}
          <Link className="link" href="/admin/imports">
            Show all imports
          </Link>
        </p>
      ) : null}
      {!validUserId ? (
        <p role="alert" className="text-error">
          User ID must be positive
        </p>
      ) : (
        <DataGrid
          rows={query.data?.items ?? []}
          columns={columns}
          isLoading={query.isLoading}
          error={query.error?.message}
          emptyMessage="No imports match these filters."
          label="Import history"
        />
      )}
      {query.data && validUserId ? (
        <Pagination
          className="mt-4"
          page={page}
          perPage={query.data.per_page}
          total={query.data.total}
          onPageChange={setPage}
        />
      ) : null}
    </ListCard>
  );
}
