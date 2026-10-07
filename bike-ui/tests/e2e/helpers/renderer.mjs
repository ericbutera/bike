import { createRequire } from "node:module";
import { fakeExternalBasemaps } from "./basemaps.mjs";

// Run the owning renderer unchanged, with only its external tile provider faked.
const require = createRequire(
  new URL("../../../../map-renderer/package.json", import.meta.url),
);
const { chromium } = require("playwright");
const launch = chromium.launch.bind(chromium);
chromium.launch = async (options) => {
  const browser = await launch(options);
  const newPage = browser.newPage.bind(browser);
  browser.newPage = async (pageOptions) => {
    const page = await newPage(pageOptions);
    await fakeExternalBasemaps(page);
    return page;
  };
  return browser;
};
await import("../../../../map-renderer/server.mjs");
