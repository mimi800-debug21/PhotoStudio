import { describe, expect, it } from "vitest";
import {
  DEFAULT_EDIT_PARAMS,
  EditHistory,
  clampParams,
  displayRectToCrop,
  isDefaultParams,
  paramsEqual,
  rotate90,
} from "./edits";
import type { EditParams } from "./types";

describe("clampParams", () => {
  it("clamps ranges", () => {
    const p = clampParams({
      crop: { x: -0.5, y: 0.9, w: 5, h: 5 },
      rotate_deg: 450,
      flip_h: true,
      flip_v: false,
      exposure: 10,
      contrast: -500,
      saturation: 100,
      brightness: 0,
      sharpen: 999,
    });
    expect(p.exposure).toBe(3);
    expect(p.contrast).toBe(-100);
    expect(p.sharpen).toBe(100);
    expect(p.rotate_deg).toBe(90);
    expect(p.crop).not.toBeNull();
    expect(p.crop!.x).toBe(0);
    expect(p.crop!.w).toBeLessThanOrEqual(1 - p.crop!.x);
    expect(p.crop!.h).toBeLessThanOrEqual(1 - p.crop!.y);
  });

  it("normalizes negative rotation", () => {
    expect(clampParams({ ...DEFAULT_EDIT_PARAMS, rotate_deg: -90 }).rotate_deg).toBe(270);
  });
});

describe("paramsEqual / isDefaultParams", () => {
  it("detects equality modulo clamping", () => {
    const a: EditParams = { ...DEFAULT_EDIT_PARAMS, exposure: 99 };
    const b: EditParams = { ...DEFAULT_EDIT_PARAMS, exposure: 3 };
    expect(paramsEqual(a, b)).toBe(true);
    expect(isDefaultParams(DEFAULT_EDIT_PARAMS)).toBe(true);
    expect(isDefaultParams({ ...DEFAULT_EDIT_PARAMS, contrast: 5 })).toBe(false);
  });
});

describe("rotate90", () => {
  it("adds quarter turns", () => {
    expect(rotate90(DEFAULT_EDIT_PARAMS, 1).rotate_deg).toBe(90);
    expect(rotate90(DEFAULT_EDIT_PARAMS, 5).rotate_deg).toBe(90);
    expect(rotate90(DEFAULT_EDIT_PARAMS, -1).rotate_deg).toBe(270);
  });
});

describe("EditHistory", () => {
  it("supports undo/redo", () => {
    const h = new EditHistory();
    expect(h.canUndo()).toBe(false);
    h.push({ ...DEFAULT_EDIT_PARAMS, exposure: 1 });
    expect(h.current.exposure).toBe(1);
    expect(h.canUndo()).toBe(true);
    h.push({ ...DEFAULT_EDIT_PARAMS, exposure: 2 });
    expect(h.undo().exposure).toBe(1);
    expect(h.canRedo()).toBe(true);
    expect(h.redo().exposure).toBe(2);
    h.push({ ...DEFAULT_EDIT_PARAMS, exposure: 0.5 });
    expect(h.canRedo()).toBe(false);
    expect(h.undo().exposure).toBe(2);
    expect(h.undo().exposure).toBe(1);
    expect(h.undo().exposure).toBe(0);
    expect(h.canUndo()).toBe(false);
  });

  it("ignores duplicate pushes", () => {
    const h = new EditHistory();
    h.push({ ...DEFAULT_EDIT_PARAMS, exposure: 1 });
    h.push({ ...DEFAULT_EDIT_PARAMS, exposure: 1 });
    expect(h.undo().exposure).toBe(0);
    expect(h.canUndo()).toBe(false);
  });
});

describe("displayRectToCrop", () => {
  it("converts display px to relative crop", () => {
    const c = displayRectToCrop(50, 25, 100, 50, 200, 100);
    expect(c).toEqual({ x: 0.25, y: 0.25, w: 0.5, h: 0.5 });
  });

  it("rejects tiny rects", () => {
    expect(displayRectToCrop(0, 0, 1, 1, 200, 100)).toBeNull();
  });
});
