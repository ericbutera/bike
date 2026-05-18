import { afterEach, describe, expect, it, vi } from "vitest";

describe("config runtime initialization", () => {
  afterEach(() => {
    vi.resetModules();
    delete window.__APP_CONFIG__;
  });

  it("lets explicit runtime config override an earlier default client config", async () => {
    const configModule = await import("./config");

    expect(configModule.getClientConfig().API_URL).toBe(
      "http://localhost:3000/api",
    );

    const resolved = configModule.initializeClientConfig({
      API_URL: "https://bike.nibelheim.dev/api",
      MAP_STYLE_URL: "street",
    });

    expect(resolved.API_URL).toBe("https://bike.nibelheim.dev/api");
    expect(resolved.MAP_STYLE_URL).toBe("street");
    expect(configModule.getClientConfig().API_URL).toBe(
      "https://bike.nibelheim.dev/api",
    );
  });

  it("refreshes stale client config from window app config when available", async () => {
    const configModule = await import("./config");

    expect(configModule.getClientConfig().API_URL).toBe(
      "http://localhost:3000/api",
    );

    window.__APP_CONFIG__ = {
      API_URL: "https://bike.nibelheim.dev/api",
      MAP_STYLE_URL: "topo",
    };

    expect(configModule.initializeClientConfig().API_URL).toBe(
      "https://bike.nibelheim.dev/api",
    );
  });
});
