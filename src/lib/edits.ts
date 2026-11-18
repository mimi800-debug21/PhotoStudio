import type { CropRect, EditParams } from "./types";

export const DEFAULT_EDIT_PARAMS: EditParams = {
  crop: null,
  rotate_deg: 0,
  flip_h: false,
  flip_v: false,
  exposure: 0,
  contrast: 0,
  saturation: 0,
  brightness: 0,
  sharpen: 0,
};

export function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

export function clampParams(p: EditParams): EditParams {
  const crop = p.crop
    ? {
        x: clamp(p.crop.x, 0, 1),
        y: clamp(p.crop.y, 0, 1),
        w: clamp(p.crop.w, 0.01, 1),
        h: clamp(p.crop.h, 0.01, 1),
      }
    : null;
  if (crop) {
    crop.w = Math.min(crop.w, 1 - crop.x);
    crop.h = Math.min(crop.h, 1 - crop.y);
  }
  return {
    crop,
    rotate_deg: ((Math.round(p.rotate_deg) % 360) + 360) % 360,
    flip_h: !!p.flip_h,
    flip_v: !!p.flip_v,
    exposure: clamp(p.exposure, -3, 3),
    contrast: clamp(p.contrast, -100, 100),
    saturation: clamp(p.saturation, -100, 100),
    brightness: clamp(p.brightness, -100, 100),
    sharpen: clamp(p.sharpen, 0, 100),
  };
}

export function paramsEqual(a: EditParams, b: EditParams): boolean {
  return JSON.stringify(clampParams(a)) === JSON.stringify(clampParams(b));
}

export function isDefaultParams(p: EditParams): boolean {
  return paramsEqual(p, DEFAULT_EDIT_PARAMS);
}

export function rotate90(p: EditParams, quarterTurns: number): EditParams {
  return clampParams({ ...p, rotate_deg: p.rotate_deg + quarterTurns * 90 });
}

/** Undo/redo history stack for edit params. */
export class EditHistory {
  private undoStack: EditParams[] = [];
  private redoStack: EditParams[] = [];

  constructor(initial: EditParams = DEFAULT_EDIT_PARAMS) {
    this.undoStack.push(initial);
  }

  get current(): EditParams {
    return this.undoStack[this.undoStack.length - 1];
  }

  /** Push a new state; clears redo stack. No-op if identical to current. */
  push(next: EditParams): EditParams {
    const cur = this.current;
    if (paramsEqual(cur, next)) return cur;
    this.undoStack.push(next);
    this.redoStack = [];
    return next;
  }

  canUndo(): boolean {
    return this.undoStack.length > 1;
  }

  canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  undo(): EditParams {
    if (!this.canUndo()) return this.current;
    const cur = this.undoStack.pop()!;
    this.redoStack.push(cur);
    return this.current;
  }

  redo(): EditParams {
    if (!this.canRedo()) return this.current;
    const next = this.redoStack.pop()!;
    this.undoStack.push(next);
    return next;
  }

  reset(to: EditParams = DEFAULT_EDIT_PARAMS): EditParams {
    this.undoStack = [to];
    this.redoStack = [];
    return to;
  }
}

/** Convert a crop rectangle drawn in image-display pixels to relative coords. */
export function displayRectToCrop(
  x: number,
  y: number,
  w: number,
  h: number,
  imgW: number,
  imgH: number,
): CropRect | null {
  if (imgW <= 0 || imgH <= 0) return null;
  const rx = x / imgW;
  const ry = y / imgH;
  const rw = w / imgW;
  const rh = h / imgH;
  if (rw < 0.02 || rh < 0.02) return null;
  return clampParams({ ...DEFAULT_EDIT_PARAMS, crop: { x: rx, y: ry, w: rw, h: rh } }).crop;
}
