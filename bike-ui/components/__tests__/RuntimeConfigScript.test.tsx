import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import RuntimeConfigScript from "../RuntimeConfigScript";

describe("runtime configuration", () => {
  it("publishes the API configuration and request context safely inside a script", () => {
    const { container } = render(
      <RuntimeConfigScript
        config={{
          API_URL: "/api",
          MAP_STYLE_URL: "fiord",
          LOCAL_ADMIN_AUTO_LOGIN: false,
        }}
        requestContext={{
          "x-request-id": "request-7",
          baggage: "ride=<ready>",
        }}
      />,
    );
    const script = container.querySelector("script")!.textContent;

    expect(script).toContain('window.__APP_CONFIG__={"API_URL":"/api"');
    expect(script).toContain(
      'window.__REQUEST_CONTEXT__={"x-request-id":"request-7"',
    );
    expect(script).toContain("ride=\\u003cready>");
    expect(script).not.toContain("<ready>");
  });
});
