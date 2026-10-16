import { render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { initializeClientConfig } from "../../lib/config";
import TemporalLink from "../ui/TemporalLink";

afterEach(() => initializeClientConfig({}));

it("uses runtime configuration and the recorded namespace and workflow identity", () => {
  initializeClientConfig({ TEMPORAL_UI_URL: "https://temporal.example.com/" });
  render(
    <TemporalLink namespace="bike tasks" workflowId="import/command 17" />,
  );
  expect(
    screen.getByRole("link", { name: "View job in Temporal" }),
  ).toHaveAttribute(
    "href",
    "https://temporal.example.com/namespaces/bike%20tasks/workflows/import%2Fcommand%2017",
  );
});

it("opens the workflow list when the domain record has no known job", () => {
  initializeClientConfig({ TEMPORAL_UI_URL: "https://temporal.example.com" });
  render(<TemporalLink />);
  expect(
    screen.getByRole("link", { name: "Open Temporal workflows" }),
  ).toHaveAttribute(
    "href",
    "https://temporal.example.com/namespaces/bike/workflows",
  );
});

it("omits the link until a public UI URL is configured", () => {
  initializeClientConfig({});
  render(<TemporalLink workflowId="command-17" />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
});
