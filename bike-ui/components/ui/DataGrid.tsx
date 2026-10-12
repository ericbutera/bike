"use client";

import type { ReactNode } from "react";

export type Column<T, P extends object = object> = {
  key: string;
  header: ReactNode;
  className?: string;
  render: (row: T, params?: P) => ReactNode;
};

export function ListCard({
  title,
  actions,
  filters,
  onSearch,
  onClear,
  children,
}: {
  title?: ReactNode;
  actions?: ReactNode;
  filters?: ReactNode;
  onSearch?: () => void;
  onClear?: () => void;
  children: ReactNode;
}) {
  return (
    <div className="card min-w-0 bg-base-100 shadow-xl">
      <div className="card-body min-w-0">
        {title || actions || filters ? (
          <div className="mb-4 flex flex-col gap-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              {title ? (
                <h2 className="card-title m-0 text-2xl font-bold">{title}</h2>
              ) : null}
              {actions ? (
                <div className="flex items-center gap-3">{actions}</div>
              ) : null}
            </div>
            {filters ? (
              <form
                className="flex w-full flex-wrap items-center justify-end gap-2 rounded-lg bg-base-200/50 p-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  onSearch?.();
                }}
              >
                <div className="flex flex-1 flex-wrap justify-end gap-2">
                  {filters}
                </div>
                <div className="join">
                  {onSearch ? (
                    <button
                      className="join-item btn btn-sm btn-neutral"
                      type="submit"
                    >
                      Search
                    </button>
                  ) : null}
                  {onClear ? (
                    <button
                      className="join-item btn btn-sm btn-ghost border-base-300"
                      type="button"
                      onClick={onClear}
                    >
                      Clear
                    </button>
                  ) : null}
                </div>
              </form>
            ) : null}
          </div>
        ) : null}
        {children}
      </div>
    </div>
  );
}

export function DataGrid<T, P extends object = object>({
  rows,
  columns,
  params,
  isLoading,
  error,
  emptyMessage,
  label,
  onRowClick,
}: {
  rows: T[];
  columns: Column<T, P>[];
  params?: P;
  isLoading?: boolean;
  error?: ReactNode;
  emptyMessage: string;
  label?: string;
  onRowClick?: (row: T) => void;
}) {
  if (isLoading)
    return <div className="p-6 text-sm text-base-content/70">Loading...</div>;
  if (error)
    return (
      <div role="alert" className="p-6 text-sm text-error">
        {error}
      </div>
    );
  if (rows.length === 0)
    return (
      <div className="p-6 text-sm text-base-content/70">{emptyMessage}</div>
    );
  return (
    <div className="overflow-x-auto">
      <table aria-label={label} className="table table-zebra w-full">
        <thead>
          <tr>
            {columns.map((column) => (
              <th className={column.className} key={column.key}>
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr
              key={String((row as { id?: string | number }).id ?? index)}
              className={onRowClick ? "cursor-pointer hover:bg-base-200" : ""}
              onClick={() => onRowClick?.(row)}
            >
              {columns.map((column) => (
                <td className={column.className} key={column.key}>
                  {column.render(row, params)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
