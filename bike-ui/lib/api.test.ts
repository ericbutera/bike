import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createFetchClient: vi.fn(),
  createClient: vi.fn(),
  fetchWithCredentials: vi.fn(),
}));

vi.mock("openapi-fetch", () => ({
  default: mocks.createFetchClient,
}));

vi.mock("openapi-react-query", () => ({
  default: mocks.createClient,
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
    vi.stubGlobal("fetch", mocks.fetchWithCredentials);
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
    vi.unstubAllGlobals();
    delete window.__APP_CONFIG__;
    delete window.__REQUEST_CONTEXT__;
  });

  it("defaults body requests to json when no content type is provided", async () => {
    const { createApiClient } = await import("./api");

    createApiClient();

    const fetchWithContext = mocks.createFetchClient.mock.calls[0]?.[0]
      ?.fetch as typeof fetch;

    await fetchWithContext("https://bike.example.com/api/oauth/oidc", {
      body: JSON.stringify({
        provider: "oidc",
        code: "authorization-code",
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

    const request = new Request("https://bike.example.com/api/oauth/oidc", {
      body: JSON.stringify({
        provider: "oidc",
        code: "authorization-code",
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
