"use client";

import { useAuth } from "@/lib/kaleido";
import { useRouter } from "next/navigation";
import type { ReactNode } from "react";
import { useEffect } from "react";

export default function RequireAdmin({ children }: { children: ReactNode }) {
  const router = useRouter();
  const { user } = useAuth() as {
    user?: { is_admin?: boolean } | null;
  };

  useEffect(() => {
    if (user === null) {
      router.replace("/login");
      return;
    }

    if (user && !user.is_admin) {
      router.replace("/");
    }
  }, [user, router]);

  if (user === undefined) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <span className="loading loading-spinner loading-lg"></span>
      </div>
    );
  }

  if (user === null || (user && !user.is_admin)) {
    return null;
  }

  return <>{children}</>;
}
