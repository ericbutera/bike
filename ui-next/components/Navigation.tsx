"use client";

import { auth } from "@ericbutera/kaleido";
import Link from "next/link";
import { useState } from "react";
import ThemeToggle from "./ThemeToggle";

type OpenMenu = "training" | "account" | null;

export default function Navigation() {
  const authApi = auth.useAuthApi();
  const { user, isLoading } = authApi.useCurrentUser();
  const logout = authApi.useLogout();
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null);
  const isAuthenticated = Boolean(user);

  const handleMenuToggle =
    (menu: Exclude<OpenMenu, null>) =>
    (event: React.ToggleEvent<HTMLDetailsElement>) => {
      const isOpen = event.currentTarget.open;

      setOpenMenu((currentMenu) => {
        if (isOpen) {
          return menu;
        }

        return currentMenu === menu ? null : currentMenu;
      });
    };

  const closeMenu = () => setOpenMenu(null);

  return (
    <div className="navbar bg-base-100 shadow-sm">
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <Link href="/" className="btn btn-ghost normal-case text-lg">
            bike
          </Link>
          {isAuthenticated ? (
            <div className="flex items-center gap-2">
              <Link href="/" className="btn btn-ghost btn-sm">
                Activities
              </Link>

              <ul className="menu menu-horizontal px-1">
                <li>
                  <details
                    open={openMenu === "training"}
                    onToggle={handleMenuToggle("training")}
                  >
                    <summary className="btn btn-ghost btn-sm">Training</summary>
                    <ul className="z-50 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg">
                      <li>
                        <Link href="/xc" onClick={closeMenu}>
                          Cross Country (XC)
                        </Link>
                      </li>
                      <li>
                        <Link href="/dh" onClick={closeMenu}>
                          Downhill (DH)
                        </Link>
                      </li>
                      <li>
                        <Link href="/segments" onClick={closeMenu}>
                          Segments
                        </Link>
                      </li>
                      <li>
                        <Link href="/fitness" onClick={closeMenu}>
                          Fitness
                        </Link>
                      </li>
                      <li>
                        <Link href="/training/reports" onClick={closeMenu}>
                          Reports
                        </Link>
                      </li>
                    </ul>
                  </details>
                </li>
              </ul>

              <ul className="menu menu-horizontal px-1">
                <li>
                  <details
                    open={openMenu === "account"}
                    onToggle={handleMenuToggle("account")}
                  >
                    <summary className="btn btn-ghost btn-sm">Account</summary>
                    <ul className="z-50 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg">
                      <li>
                        <Link href="/account" onClick={closeMenu}>
                          Account
                        </Link>
                      </li>
                      {user?.is_admin ? (
                        <li>
                          <Link href="/admin" onClick={closeMenu}>
                            Admin
                          </Link>
                        </li>
                      ) : null}
                      <li>
                        <ThemeToggle />
                      </li>
                      <li>
                        <button
                          type="button"
                          onClick={() => {
                            closeMenu();
                            logout.mutateAsync();
                          }}
                          disabled={logout.isPending}
                        >
                          {logout.isPending ? "Signing out..." : "Sign out"}
                        </button>
                      </li>
                    </ul>
                  </details>
                </li>
              </ul>
            </div>
          ) : null}
        </div>
      </div>
      {!isLoading && !isAuthenticated ? (
        <div className="flex-none">
          <Link href="/login" className="btn btn-primary btn-sm">
            Sign in
          </Link>
        </div>
      ) : null}
    </div>
  );
}
