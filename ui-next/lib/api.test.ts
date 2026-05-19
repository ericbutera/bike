import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createFetchClient: vi.fn(),
  createClient: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  createFetchClient: mocks.createFetchClient,
  createClient: mocks.createClient,
}));

describe("api runtime client", () => {
  beforeEach(() => {
    vi.resetModules();
    mocks.createFetchClient.mockReset();
    mocks.createClient.mockReset();
    delete window.__APP_CONFIG__;

    mocks.createFetchClient.mockImplementation(({ baseUrl }) => ({ baseUrl }));
    mocks.createClient.mockImplementation((fetchClient) => ({
      useQuery: vi.fn(() => fetchClient.baseUrl),
    }));
  });

  afterEach(() => {
    vi.resetModules();
    delete window.__APP_CONFIG__;
  });

  it("creates the client lazily using the resolved runtime api url", async () => {
    window.__APP_CONFIG__ = {
      API_URL: "https://bike.nibelheim.dev/api",
      MAP_STYLE_URL: "topo",
    };

    const { $api } = await import("./api");

    expect(mocks.createFetchClient).not.toHaveBeenCalled();

    const useQuery = Reflect.get($api as object, "useQuery");

    expect(typeof useQuery).toBe("function");
    (useQuery as (...args: unknown[]) => unknown)();

    expect(mocks.createFetchClient).toHaveBeenCalledWith({
      baseUrl: "https://bike.nibelheim.dev/api",
    });
  });
});
