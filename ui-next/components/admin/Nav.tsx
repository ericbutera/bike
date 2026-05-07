"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

export default function Nav() {
  const pathname = usePathname();

  const linkClass = (target: string, exact = false) => {
    const isActive = exact ? pathname === target : pathname.startsWith(target);
    return isActive ? "menu-active font-semibold" : undefined;
  };

  return (
    <ul className="menu p-2 w-full bg-base-100 text-base-content">
      <li>
        <Link href="/admin" className={linkClass("/admin", true)}>
          Overview
        </Link>
      </li>
      <li>
        <Link href="/admin/tasks" className={linkClass("/admin/tasks")}>
          Tasks
        </Link>
      </li>
      <li>
        <Link
          href="/admin/feature-flags"
          className={linkClass("/admin/feature-flags")}
        >
          Feature Flags
        </Link>
      </li>
      <li>
        <Link href="/admin/users" className={linkClass("/admin/users")}>
          Users
        </Link>
      </li>
      <li>
        <Link href="/admin/metrics" className={linkClass("/admin/metrics")}>
          Metrics
        </Link>
      </li>
    </ul>
  );
}
