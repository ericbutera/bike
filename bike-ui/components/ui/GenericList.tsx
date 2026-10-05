"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import Pagination from "./Pagination";
import { useKeyedState } from "../../lib/useKeyedState";

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
  const [localParams, setLocalParams] = useKeyedState(search, queryParams);
  const [pendingNavigation, setPendingNavigation] = useState<{
    search: string;
    href: string;
  } | null>(null);

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

  useEffect(() => {
    if (!pendingNavigation || pendingNavigation.search !== search) {
      return undefined;
    }

    const timeout = setTimeout(() => {
      router.replace(pendingNavigation.href, { scroll: false });
    }, 300);
    return () => clearTimeout(timeout);
  }, [pendingNavigation, router, search]);

  const setFilter = useCallback(
    (key: string, value: unknown) => {
      const updated = {
        ...localParams,
        [key]: value === "" ? undefined : value,
      } as P & { page?: number };
      if (key !== "page") updated.page = 1;

      setLocalParams(updated);
      setPendingNavigation({ search, href: hrefFor(updated) });
    },
    [hrefFor, localParams, search, setLocalParams],
  );

  const handleSearch = () => {
    setPendingNavigation(null);
    router.push(hrefFor(localParams), { scroll: false });
  };

  const handleClear = () => {
    setPendingNavigation(null);
    const cleared = {} as P;
    setLocalParams(cleared);
    router.push(pathname, { scroll: false });
  };

  const handlePageChange = (nextPage: number) => {
    setPendingNavigation(null);
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
