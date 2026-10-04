import type { ReactNode } from "react";
import Navigation from "./Navigation";

export default function Layout({
  children,
  mainClassName,
}: {
  children: ReactNode;
  mainClassName?: string;
}) {
  return (
    <div className="min-h-screen bg-base-200">
      <Navigation />
      <main className={mainClassName}>{children}</main>
    </div>
  );
}
