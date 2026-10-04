import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import FeatureFlagsPanel from "../admin/FeatureFlagsPanel";

const mocks = vi.hoisted(() => ({
  useAdminFeatureFlags: vi.fn(),
  useUpdateFeatureFlag: vi.fn(),
  updateAsync: vi.fn(),
}));

vi.mock("@/lib/queries", () => ({
  useAdminFeatureFlags: mocks.useAdminFeatureFlags,
  useUpdateFeatureFlag: mocks.useUpdateFeatureFlag,
}));

describe("FeatureFlagsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useAdminFeatureFlags.mockReturnValue({
      data: [
        {
          feature_key: "activity_list_full_maps",
          enabled: true,
          description: "Show full maps in activity lists",
        },
        {
          feature_key: "experimental_reports",
          enabled: false,
          description: null,
        },
      ],
      isLoading: false,
      isError: false,
    });
    mocks.updateAsync.mockResolvedValue({
      feature_key: "activity_list_full_maps",
      enabled: false,
    });
    mocks.useUpdateFeatureFlag.mockReturnValue({
      updateAsync: mocks.updateAsync,
    });
  });

  it("lists flags with their current state", () => {
    render(<FeatureFlagsPanel />);

    expect(screen.getByText("activity_list_full_maps")).toBeInTheDocument();
    expect(screen.getByText("experimental_reports")).toBeInTheDocument();
    expect(screen.getByText("Enabled")).toBeInTheDocument();
    expect(screen.getByText("Disabled")).toBeInTheDocument();
  });

  it("autosaves a toggle through the admin mutation", async () => {
    const user = userEvent.setup();
    render(<FeatureFlagsPanel />);

    await user.click(
      screen.getByRole("checkbox", {
        name: "Toggle activity_list_full_maps",
      }),
    );

    await waitFor(() => {
      expect(mocks.updateAsync).toHaveBeenCalledWith(
        "activity_list_full_maps",
        false,
      );
    });
  });
});
