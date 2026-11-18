import { describe, expect, it } from "vitest";
import { formatBytes, formatDateGroup, formatDateTime, formatDuration, shortPath } from "./format";

describe("formatBytes", () => {
  it("formats sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe("3.0 GB");
    expect(formatBytes(null)).toBe("—");
  });
});

describe("formatDateTime", () => {
  it("handles null and ISO", () => {
    expect(formatDateTime(null)).toBe("Unknown date");
    expect(formatDateTime("2024-05-01 10:20:30")).toBe("2024-05-01 10:20:30");
    expect(formatDateTime("2024-05-01T10:20:30")).toBe("2024-05-01 10:20:30");
  });
});

describe("formatDateGroup", () => {
  it("extracts date", () => {
    expect(formatDateGroup("2024-05-01 10:20:30")).toBe("2024-05-01");
    expect(formatDateGroup(null)).toBe("Unknown date");
  });
});

describe("formatDuration", () => {
  it("formats durations", () => {
    expect(formatDuration(500)).toBe("500 ms");
    expect(formatDuration(2500)).toBe("2.5 s");
    expect(formatDuration(125000)).toBe("2m 5s");
  });
});

describe("shortPath", () => {
  it("truncates long paths", () => {
    const long = "C:\\Users\\someone\\Pictures\\Vacation 2024\\Photos\\IMG_1234.jpg";
    const s = shortPath(long, 30);
    expect(s.length).toBeLessThanOrEqual(31);
    expect(s).toContain("…");
    expect(shortPath("short.jpg", 30)).toBe("short.jpg");
  });
});
