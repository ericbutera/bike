"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import Pagination from "./Pagination";

export type Column<T, P extends object> = {
  key: string;
  header: ReactNode;
  className?: string;
  render: (row: T, params?: P) => ReactNode;
};

type ListMetadata = {
  page?: number;
  per_page?: number;
  total?: number;
};

function paramsFromSearch<P extends object>(search: string): P {
  const params: Record<string, unknown> = {};
  new URLSearchParams(search).forEach((value, key) => {
    if (key === "page" || key === "per_page") {
      const parsed = Number(value);
      params[key] = Number.isFinite(parsed) ? parsed : value;
    } else {
      params[key] = value;
    }
  });
  return params as P;
}

function searchFromParams(params: object) {
  const search = new URLSearchParams();
  Object.entries(params).forEach(([key, value]) => {
    if (value === undefined || value === null || value === "") return;
    search.set(key, String(value));
  });
  return search.toString();
}

export default function GenericList<T, P extends object>({
  title,
  actions,
  useQuery,
  columns,
  renderFilters,
  emptyMessage,
  onRowClick,
}: {
  title?: ReactNode;
  actions?: ReactNode;
  useQuery: (params: P) => {
    data?: T[];
    isLoading?: boolean;
    raw?: { metadata?: ListMetadata };
  };
  columns: Column<T, P>[];
  renderFilters?: (
    params: P,
    setFilter: (key: string, value: unknown) => void,
  ) => ReactNode;
  emptyMessage: string;
  paramsSchema?: unknown;
  onRowClick?: (row: T) => void;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const search = searchParams.toString();
  const queryParams = useMemo(() => paramsFromSearch<P>(search), [search]);
  const [localParams, setLocalParams] = useState<P>(queryParams);
  const pendingNavigation = useRef<ReturnType<typeof setTimeout> | null>(null);

  const result = useQuery(queryParams);
  const rows = result.data ?? [];
  const metadata = result.raw?.metadata ?? {};
  const paginationParams = queryParams as P & {
    page?: number | string;
    per_page?: number | string;
  };
  const total = metadata.total ?? rows.length;
  const page = Number(metadata.page ?? paginationParams.page) || 1;
  const perPage = Number(metadata.per_page ?? paginationParams.per_page) || 20;

  const hrefFor = useCallback(
    (params: P) => {
      const query = searchFromParams(params);
      return query ? `${pathname}?${query}` : pathname;
    },
    [pathname],
  );

  const cancelPendingNavigation = useCallback(() => {
    if (pendingNavigation.current !== null) {
      clearTimeout(pendingNavigation.current);
      pendingNavigation.current = null;
    }
  }, []);

  useEffect(() => {
    cancelPendingNavigation();
    setLocalParams(queryParams);
  }, [cancelPendingNavigation, queryParams]);

  useEffect(() => {
    return () => cancelPendingNavigation();
  }, [cancelPendingNavigation]);

  const setFilter = (key: string, value: unknown) => {
    const updated = {
      ...localParams,
      [key]: value === "" ? undefined : value,
    } as P & { page?: number };
    if (key !== "page") updated.page = 1;

    setLocalParams(updated);
    cancelPendingNavigation();
    pendingNavigation.current = setTimeout(() => {
      router.replace(hrefFor(updated), { scroll: false });
      pendingNavigation.current = null;
    }, 300);
  };

  const handleSearch = () => {
    cancelPendingNavigation();
    router.push(hrefFor(localParams), { scroll: false });
  };

  const handleClear = () => {
    cancelPendingNavigation();
    const cleared = {} as P;
    setLocalParams(cleared);
    router.push(pathname, { scroll: false });
  };

  const handlePageChange = (nextPage: number) => {
    cancelPendingNavigation();
    const updated = {
      ...localParams,
      page: nextPage,
    } as P;
    setLocalParams(updated);
    router.push(hrefFor(updated), { scroll: false });
  };

  const showHeader = title || actions || renderFilters;
  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        {showHeader ? (
          <div className="mb-4 flex flex-col gap-4">
            <div className="flex items-center justify-between">
              {title ? (
                <div className="flex items-center gap-3">
                  <h2 className="card-title m-0 text-2xl font-bold">{title}</h2>
                </div>
              ) : null}
              {actions ? (
                <div className="flex items-center gap-3">{actions}</div>
              ) : null}
            </div>
            {renderFilters ? (
              <div className="flex w-full flex-wrap items-center justify-end gap-2 rounded-lg bg-base-200/50 p-2">
                <div className="flex flex-1 flex-wrap justify-end gap-2">
                  {renderFilters(localParams, setFilter)}
                </div>
                <div className="join">
                  <button
                    className="join-item btn btn-sm btn-neutral"
                    type="button"
                    onClick={handleSearch}
                  >
                    Search
                  </button>
                  <button
                    className="join-item btn btn-sm btn-ghost border-base-300"
                    type="button"
                    onClick={handleClear}
                  >
                    Clear
                  </button>
                </div>
              </div>
            ) : null}
          </div>
        ) : null}
        {result.isLoading ? (
          <div className="p-6 text-sm text-base-content/70">Loading...</div>
        ) : rows.length === 0 ? (
          <div className="p-6 text-sm text-base-content/70">{emptyMessage}</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="table table-zebra w-full">
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
                {rows.map((row, index) => {
                  const id = (row as { id?: string | number }).id;
                  const rowKey = id === undefined ? `row-${index}` : String(id);
                  return (
                    <tr
                      key={rowKey}
                      className={
                        onRowClick ? "cursor-pointer hover:bg-base-200" : ""
                      }
                      onClick={() => onRowClick?.(row)}
                    >
                      {columns.map((column) => (
                        <td className={column.className} key={column.key}>
                          {column.render(row, localParams)}
                        </td>
                      ))}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
        {total > 0 && perPage > 0 ? (
          <div className="mt-4">
            <Pagination
              page={page}
              perPage={perPage}
              total={total}
              onPageChange={handlePageChange}
            />
          </div>
        ) : null}
      </div>
    </div>
  );
}
