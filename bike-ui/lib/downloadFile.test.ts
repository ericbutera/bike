import { afterEach, describe, expect, it, vi } from "vitest";
import { downloadFile } from "./downloadFile";

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("downloadFile", () => {
  it("clicks a temporary download link and releases its URL afterward", () => {
    vi.useFakeTimers();
    const blob = new Blob(["route"], { type: "application/gpx+xml" });
    const createUrl = vi
      .spyOn(URL, "createObjectURL")
      .mockReturnValue("blob:segment");
    const revokeUrl = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => {});
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(function (this: HTMLAnchorElement) {
        expect(this).toHaveAttribute("href", "blob:segment");
        expect(this).toHaveAttribute("download", "segment-14.gpx");
        expect(this).toBeInTheDocument();
        expect(revokeUrl).not.toHaveBeenCalled();
      });

    downloadFile("segment-14.gpx", blob);

    expect(createUrl).toHaveBeenCalledWith(blob);
    expect(click).toHaveBeenCalledOnce();
    expect(document.querySelector("a[download]")).toBeNull();
    expect(revokeUrl).not.toHaveBeenCalled();
    vi.runAllTimers();
    expect(revokeUrl).toHaveBeenCalledExactlyOnceWith("blob:segment");
  });

  it("cleans up the link and URL if starting the download fails", () => {
    vi.useFakeTimers();
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:segment");
    const revokeUrl = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {
      throw new Error("Download failed");
    });

    expect(() => downloadFile("segment-14.gpx", new Blob())).toThrow(
      "Download failed",
    );
    expect(document.querySelector("a[download]")).toBeNull();
    vi.runAllTimers();
    expect(revokeUrl).toHaveBeenCalledExactlyOnceWith("blob:segment");
  });
});
