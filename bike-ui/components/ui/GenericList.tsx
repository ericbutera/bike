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
import { DataGrid, ListCard, type Column } from "./DataGrid";
import { useKeyedState } from "../../lib/useKeyedState";

export type { Column } from "./DataGrid";

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
    error?: unknown;
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

  return (
    <ListCard
      title={title}
      actions={actions}
      filters={renderFilters?.(localParams, setFilter)}
      onSearch={handleSearch}
      onClear={handleClear}
    >
      <DataGrid
        rows={rows}
        columns={columns}
        params={localParams}
        isLoading={result.isLoading}
        error={
          result.error instanceof Error
            ? result.error.message
            : result.error
              ? "Unable to load this list."
              : undefined
        }
        emptyMessage={emptyMessage}
        label={typeof title === "string" ? title : undefined}
        onRowClick={onRowClick}
      />
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
    </ListCard>
  );
}
