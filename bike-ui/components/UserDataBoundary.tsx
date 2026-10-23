"use client";

import {
  matchQuery,
  useQueryClient,
  type QueryFilters,
} from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { CURRENT_USER_QUERY_KEY, useAuth } from "../lib/auth";
import { LoadingCard } from "./ui/QueryState";

const applicationQueries: QueryFilters = {
  predicate: (query) =>
    !matchQuery({ queryKey: CURRENT_USER_QUERY_KEY }, query),
};

export default function UserDataBoundary({
  children,
}: {
  children: ReactNode;
}) {
  const client = useQueryClient();
  const { user, isLoading } = useAuth();
  const owner = user ? String(user.id) : null;
  const [cacheOwner, setCacheOwner] = useState<string | null | undefined>();

  useEffect(() => {
    if (isLoading || cacheOwner === owner) return;
    let active = true;
    void client.cancelQueries(applicationQueries).then(() => {
      if (!active) return;
      client.removeQueries(applicationQueries);
      setCacheOwner(owner);
    });
    return () => {
      active = false;
    };
  }, [cacheOwner, client, isLoading, owner]);

  if (isLoading || cacheOwner !== owner) return <LoadingCard />;
  return children;
}
