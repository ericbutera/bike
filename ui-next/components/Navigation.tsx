"use client";

import { auth } from "@ericbutera/kaleido";
import Link from "next/link";
import ThemeToggle from "./ThemeToggle";

export default function Navigation() {
  const authApi = auth.useAuthApi();
  const { user, isLoading } = authApi.useCurrentUser();
  const logout = authApi.useLogout();

  return (
    <div className="navbar bg-base-100 shadow-sm">
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <Link href="/" className="btn btn-ghost normal-case text-lg">
            bike
          </Link>
          <div className="flex items-center gap-2">
            <Link href="/" className="btn btn-ghost btn-sm">
              Activities
            </Link>

            <ul className="menu menu-horizontal px-1">
              <li>
                <details>
                  <summary className="btn btn-ghost btn-sm">Training</summary>
                  <ul className="z-50 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg">
                    <li>
                      <Link href="/xc">Cross Country (XC)</Link>
                    </li>
                    <li>
                      <Link href="/dh">Downhill (DH)</Link>
                    </li>
                    <li>
                      <Link href="/segments">Segments</Link>
                    </li>
                    <li>
                      <Link href="/fitness">Fitness</Link>
                    </li>
                    <li>
                      <Link href="/training/reports">Reports</Link>
                    </li>
                  </ul>
                </details>
              </li>
            </ul>

            <ul className="menu menu-horizontal px-1">
              <li>
                <details>
                  <summary className="btn btn-ghost btn-sm">Account</summary>
                  <ul className="z-50 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg">
                    <li>
                      <Link href="/account">Account</Link>
                    </li>
                    {user?.is_admin ? (
                      <li>
                        <Link href="/admin">Admin</Link>
                      </li>
                    ) : null}
                    <li>
                      <ThemeToggle />
                    </li>
                    {isLoading ? null : user ? (
                      <li>
                        <button
                          type="button"
                          onClick={() => logout.mutateAsync()}
                          disabled={logout.isPending}
                        >
                          {logout.isPending ? "Signing out..." : "Sign out"}
                        </button>
                      </li>
                    ) : (
                      <li>
                        <Link href="/login">Login</Link>
                      </li>
                    )}
                  </ul>
                </details>
              </li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}
