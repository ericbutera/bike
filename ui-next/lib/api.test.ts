import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createFetchClient: vi.fn(),
  createClient: vi.fn(),
  fetchWithCredentials: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  createFetchClient: mocks.createFetchClient,
  createClient: mocks.createClient,
  fetchWithCredentials: mocks.fetchWithCredentials,
}));

describe("api runtime client", () => {
  beforeEach(() => {
    vi.resetModules();
    mocks.createFetchClient.mockReset();
    mocks.createClient.mockReset();
    mocks.fetchWithCredentials.mockReset();
    delete window.__APP_CONFIG__;
    delete window.__REQUEST_CONTEXT__;

    mocks.createFetchClient.mockImplementation(({ baseUrl }) => ({ baseUrl }));
    mocks.fetchWithCredentials.mockResolvedValue(new Response("{}"));
    mocks.createClient.mockImplementation((fetchClient) => ({
      useQuery: vi.fn(() => fetchClient.baseUrl),
    }));
  });

  function forwardedRequest() {
    const input = mocks.fetchWithCredentials.mock.calls[0]?.[0];

    expect(input).toBeInstanceOf(Request);

    return input as Request;
  }

  afterEach(() => {
    vi.resetModules();
    delete window.__APP_CONFIG__;
    delete window.__REQUEST_CONTEXT__;
  });

  it("creates the client lazily using the resolved runtime api url", async () => {
    window.__APP_CONFIG__ = {
      API_URL: "https://bike.example.com/api",
      MAP_STYLE_URL: "topo",
    };

    const { $api } = await import("./api");

    expect(mocks.createFetchClient).not.toHaveBeenCalled();

    const useQuery = Reflect.get($api as object, "useQuery");

    expect(typeof useQuery).toBe("function");
    (useQuery as (...args: unknown[]) => unknown)();

    expect(mocks.createFetchClient).toHaveBeenCalledWith(
      expect.objectContaining({
        baseUrl: "https://bike.example.com/api",
        fetch: expect.any(Function),
      }),
    );
  });

  it("forwards the browser request context through the api fetch client", async () => {
    window.__REQUEST_CONTEXT__ = {
      traceparent: "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
      "x-request-id": "ingress-request-7",
    };

    const { createApiClient } = await import("./api");

    createApiClient();

    const fetchWithContext = mocks.createFetchClient.mock.calls[0]?.[0]
      ?.fetch as typeof fetch;

    await fetchWithContext("https://bike.example.com/api/preferences", {
      headers: { Accept: "application/json" },
    });

    const request = forwardedRequest();

    expect(request.headers.get("accept")).toBe("application/json");
    expect(request.headers.get("traceparent")).toBe(
      "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    );
    expect(request.headers.get("x-request-id")).toBe("ingress-request-7");
  });

  it("defaults body requests to json when no content type is provided", async () => {
    const { createApiClient } = await import("./api");

    createApiClient();

    const fetchWithContext = mocks.createFetchClient.mock.calls[0]?.[0]
      ?.fetch as typeof fetch;

    await fetchWithContext("https://bike.example.com/api/auth/login", {
      body: JSON.stringify({
        email: "rider@example.com",
        password: "secret-password",
      }),
      method: "POST",
    });

    const request = forwardedRequest();

    expect(request.method).toBe("POST");
    expect(request.headers.get("content-type")).toBe("application/json");
  });

  it("preserves headers from Request inputs built by the openapi client", async () => {
    const { createApiClient } = await import("./api");

    createApiClient();

    const fetchWithContext = mocks.createFetchClient.mock.calls[0]?.[0]
      ?.fetch as typeof fetch;

    const request = new Request("https://bike.example.com/api/auth/login", {
      body: JSON.stringify({
        email: "rider@example.com",
        password: "secret-password",
      }),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });

    await fetchWithContext(request);

    const forwarded = forwardedRequest();

    expect(forwarded.method).toBe("POST");
    expect(forwarded.headers.get("content-type")).toBe("application/json");
  });
});
