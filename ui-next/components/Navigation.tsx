"use client";

import { auth } from "@ericbutera/kaleido";
import Link from "next/link";
import ThemeToggle from "./ThemeToggle";

const NAV_MENU_ID = "bike-nav-menu";
const TRAINING_MENU_ID = "bike-nav-training-menu";
const ACCOUNT_MENU_ID = "bike-nav-account-menu";
const NAV_POPOVER_IDS = [NAV_MENU_ID, TRAINING_MENU_ID, ACCOUNT_MENU_ID];

const trainingLinks = [
  { href: "/xc", label: "Cross Country (XC)" },
  { href: "/dh", label: "Downhill (DH)" },
  { href: "/segments", label: "Segments" },
  { href: "/segments/progress", label: "Segment Progress" },
  { href: "/segments/analysis", label: "Segment Analysis" },
  { href: "/fitness", label: "Fitness" },
  { href: "/training/reports", label: "Reports" },
];

function closeNavPopovers() {
  for (const id of NAV_POPOVER_IDS) {
    const element = document.getElementById(id);

    if (!element || typeof element.hidePopover !== "function") {
      continue;
    }

    try {
      element.hidePopover();
    } catch {
      // Browsers throw if hidePopover is called while the popover is closed.
    }
  }
}

export default function Navigation() {
  const authApi = auth.useAuthApi();
  const { user, isLoading } = authApi.useCurrentUser();
  const logout = authApi.useLogout();
  const isAuthenticated = Boolean(user);

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

              <button
                type="button"
                className="btn btn-ghost btn-sm sm:hidden"
                popoverTarget={NAV_MENU_ID}
              >
                Menu
              </button>

              <div
                id={NAV_MENU_ID}
                className="megamenu megamenu-sm megamenu-wide max-sm:megamenu-vertical z-50 border border-base-300 bg-base-100 p-2 sm:border-0 sm:bg-transparent sm:p-0"
                popover="auto"
              >
                <span className="megamenu-active"></span>

                <button type="button" popoverTarget={TRAINING_MENU_ID}>
                  Training
                </button>
                <div
                  id={TRAINING_MENU_ID}
                  className="z-50 w-72 max-w-[calc(100vw-1rem)] p-2 shadow-lg sm:w-[34rem]"
                  popover="auto"
                >
                  <ul className="menu w-full sm:menu-horizontal">
                    <li>
                      <ul>
                        <li className="menu-title">Ride Focus</li>
                        {trainingLinks.slice(0, 2).map((link) => (
                          <li key={link.href}>
                            <Link href={link.href} onClick={closeNavPopovers}>
                              {link.label}
                            </Link>
                          </li>
                        ))}
                      </ul>
                    </li>
                    <li>
                      <ul>
                        <li className="menu-title">Segments</li>
                        {trainingLinks.slice(2, 5).map((link) => (
                          <li key={link.href}>
                            <Link href={link.href} onClick={closeNavPopovers}>
                              {link.label}
                            </Link>
                          </li>
                        ))}
                      </ul>
                    </li>
                    <li>
                      <ul>
                        <li className="menu-title">Insights</li>
                        {trainingLinks.slice(5).map((link) => (
                          <li key={link.href}>
                            <Link href={link.href} onClick={closeNavPopovers}>
                              {link.label}
                            </Link>
                          </li>
                        ))}
                      </ul>
                    </li>
                  </ul>
                </div>

                <button type="button" popoverTarget={ACCOUNT_MENU_ID}>
                  Account
                </button>
                <div
                  id={ACCOUNT_MENU_ID}
                  className="z-50 w-56 max-w-[calc(100vw-1rem)] p-2 shadow-lg"
                  popover="auto"
                >
                  <ul className="menu w-full">
                    <li>
                      <Link href="/account" onClick={closeNavPopovers}>
                        Account
                      </Link>
                    </li>
                    {user?.is_admin ? (
                      <li>
                        <Link href="/admin" onClick={closeNavPopovers}>
                          Admin
                        </Link>
                      </li>
                    ) : null}
                    <li onClick={closeNavPopovers}>
                      <ThemeToggle />
                    </li>
                    <li>
                      <button
                        type="button"
                        onClick={() => {
                          closeNavPopovers();
                          void logout.mutateAsync();
                        }}
                        disabled={logout.isPending}
                      >
                        {logout.isPending ? "Signing out..." : "Sign out"}
                      </button>
                    </li>
                  </ul>
                </div>
              </div>
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
