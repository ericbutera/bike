"use client";

import { useIsFetching, useIsMutating } from "@tanstack/react-query";
import type { ComponentProps, ReactNode } from "react";
import { AppCard } from "./Card";

type SpinnerSize = "xs" | "sm" | "md" | "lg";

function cx(...classNames: Array<string | false | null | undefined>) {
  return classNames.filter(Boolean).join(" ");
}

function LoadingSpinner({
  size = "md",
  className,
  ...props
}: ComponentProps<"span"> & {
  size?: SpinnerSize;
  className?: string;
}) {
  return (
    <span
      className={cx("loading loading-spinner", `loading-${size}`, className)}
      {...props}
    />
  );
}

function CenteredLoading({
  size = "md",
  className,
}: {
  size?: SpinnerSize;
  className?: string;
}) {
  return (
    <div className={cx("flex justify-center py-8", className)}>
      <LoadingSpinner size={size} />
    </div>
  );
}

function QueryStateCard({
  children,
  className,
  bodyClassName,
}: {
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <AppCard as="section" className={className} bodyClassName={bodyClassName}>
      {children}
    </AppCard>
  );
}

function LoadingCard({
  size = "md",
  className,
  bodyClassName = "items-center py-10",
}: {
  size?: SpinnerSize;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <QueryStateCard className={className} bodyClassName={bodyClassName}>
      <LoadingSpinner size={size} />
    </QueryStateCard>
  );
}

function ErrorCard({
  fallback = "Request failed",
  className,
  bodyClassName,
}: {
  error?: unknown;
  fallback?: string;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <QueryStateCard className={className} bodyClassName={bodyClassName}>
      <div className="alert alert-error">
        <span>{fallback}</span>
      </div>
    </QueryStateCard>
  );
}

function ReactQueryActivityIndicator({
  className = "progress progress-primary fixed left-0 right-0 top-0 z-[100] h-1 w-full rounded-none",
  includeMutations = true,
}: {
  className?: string;
  includeMutations?: boolean;
}) {
  const fetchingCount = useIsFetching();
  const mutatingCount = useIsMutating();
  const activeCount = fetchingCount + (includeMutations ? mutatingCount : 0);

  if (activeCount === 0) {
    return null;
  }

  return <progress className={className} aria-label="Loading" />;
}

export {
  CenteredLoading,
  ErrorCard,
  LoadingCard,
  LoadingSpinner,
  QueryStateCard,
  ReactQueryActivityIndicator,
};
