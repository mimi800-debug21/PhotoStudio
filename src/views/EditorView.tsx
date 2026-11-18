import { useCallback, useEffect, useRef, useState } from "react";
import { useApp } from "../store";
import { api, imageUrl, previewUrl } from "../lib/api";
import { DEFAULT_EDIT_PARAMS, EditHistory, clampParams, displayRectToCrop, isDefaultParams, rotate90 } from "../lib/edits";
import type { EditParams, Photo } from "../lib/types";

function Slider({
  label,
  value,
  min,
  max,
  step,
  format,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format?: (v: number) => string;
  onChange: (v: number) => void;
}) {
  return (
    <label className="block">
      <div className="mb-1 flex items-center justify-between text-xs">
        <span className="text-neutral-400">{label}</span>
        <span className="font-mono text-neutral-300">{format ? format(value) : value.toFixed(step < 1 ? 2 : 0)}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </label>
  );
}

export default function EditorView() {
  const { selectedId, setRoute, refreshPhotos } = useApp();
  const [photo, setPhoto] = useState<Photo | null>(null);
  const [params, setParams] = useState<EditParams>(DEFAULT_EDIT_PARAMS);
  const [history] = useState(() => new EditHistory());
  const [preview, setPreview] = useState<string | null>(null);
  const [original, setOriginal] = useState<string | null>(null);
  const [showOriginal, setShowOriginal] = useState(false);
  const [cropMode, setCropMode] = useState(false);
  const [cropDrag, setCropDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [exportMsg, setExportMsg] = useState<string | null>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const paramsRef = useRef(params);
  paramsRef.current = params;

  // load photo + saved edits
  useEffect(() => {
    if (selectedId === null) return;
    let cancelled = false;
    (async () => {
      try {
        const p = await api.getPhoto(selectedId);
        const saved = await api.getEdits(selectedId);
        if (cancelled) return;
        setPhoto(p);
        const init = clampParams(saved ?? DEFAULT_EDIT_PARAMS);
        history.reset(init);
        setParams(init);
        setOriginal(await imageUrl(selectedId));
      } catch (e) {
        setBusy(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  // debounced preview render
  useEffect(() => {
    if (selectedId === null) return;
    const t = setTimeout(() => {
      previewUrl(selectedId, paramsRef.current, 1600)
        .then((u) => {
          setPreview((old) => {
            if (old) URL.revokeObjectURL(old);
            return u;
          });
        })
        .catch((e) => setBusy(String(e)));
      void api.saveEdits(selectedId, paramsRef.current).catch(() => {});
    }, 220);
    return () => clearTimeout(t);
  }, [params, selectedId]);

  // keyboard shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        setParams(e.shiftKey ? history.redo() : history.undo());
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
        e.preventDefault();
        setParams(history.redo());
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [history]);

  const update = useCallback((next: EditParams) => {
    setParams(history.push(clampParams(next)));
  }, [history]);

  const doExport = async () => {
    if (!photo) return;
    setExportMsg(null);
    try {
      const base = photo.filename.replace(/\.[^.]+$/, "");
      const out = await api.pickSaveFile(`${base}_edited.jpg`, undefined);
      if (!out) return;
      setBusy("Exporting…");
      const rec = await api.exportPhoto(photo.id, paramsRef.current, out);
      setBusy(null);
      setExportMsg(`Saved to ${rec.out_path}`);
      setTimeout(() => setExportMsg(null), 8000);
      await refreshPhotos();
    } catch (e) {
      setBusy(null);
      setExportMsg(`Export failed: ${e}`);
    }
  };

  if (selectedId === null || !photo) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center text-sm text-neutral-500">
          {selectedId === null ? (
            <>
              Open a photo from the{" "}
              <button className="text-brand-400 underline" onClick={() => setRoute("library")}>
                Library
              </button>{" "}
              to edit it.
            </>
          ) : (
            busy ?? "Loading…"
          )}
        </div>
      </div>
    );
  }

  const imgBox = imgRef.current?.getBoundingClientRect();
  const rect =
    cropDrag && imgBox
      ? {
          x: Math.min(cropDrag.x0, cropDrag.x1),
          y: Math.min(cropDrag.y0, cropDrag.y1),
          w: Math.abs(cropDrag.x1 - cropDrag.x0),
          h: Math.abs(cropDrag.y1 - cropDrag.y0),
        }
      : null;

  const toImgCoords = (e: React.MouseEvent) => {
    const box = imgRef.current?.getBoundingClientRect();
    if (!box) return null;
    return { x: e.clientX - box.left, y: e.clientY - box.top, w: box.width, h: box.height };
  };

  return (
    <div className="flex h-full">
      {/* canvas */}
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 items-center gap-2 border-b border-neutral-800 px-3 py-1.5 text-xs">
          <span className="max-w-xs truncate font-medium">{photo.filename}</span>
          <button
            className={`rounded px-2 py-1 ${showOriginal ? "bg-brand-600 text-white" : "bg-neutral-800 hover:bg-neutral-700"}`}
            onClick={() => setShowOriginal((v) => !v)}
            title="Toggle before/after"
          >
            {showOriginal ? "Showing: Original" : "Showing: Edited"}
          </button>
          <button
            className={`rounded px-2 py-1 ${cropMode ? "bg-brand-600 text-white" : "bg-neutral-800 hover:bg-neutral-700"}`}
            onClick={() => setCropMode((v) => !v)}
          >
            Crop
          </button>
          <div className="ml-auto flex items-center gap-1">
            <button
              className="rounded bg-neutral-800 px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
              disabled={!history.canUndo()}
              onClick={() => setParams(history.undo())}
              title="Undo (Ctrl+Z)"
            >
              ↶ Undo
            </button>
            <button
              className="rounded bg-neutral-800 px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
              disabled={!history.canRedo()}
              onClick={() => setParams(history.redo())}
              title="Redo (Ctrl+Shift+Z)"
            >
              ↷ Redo
            </button>
            <button
              className="rounded bg-neutral-800 px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
              disabled={isDefaultParams(params)}
              onClick={() => update(DEFAULT_EDIT_PARAMS)}
              title="Reset all adjustments"
            >
              Reset
            </button>
          </div>
        </div>

        <div className="relative min-h-0 flex-1 overflow-auto bg-neutral-950/60 p-4">
          <div className="flex h-full items-center justify-center">
            <div className="editor-canvas-wrap relative inline-block">
              <img
                ref={imgRef}
                src={showOriginal ? original ?? undefined : preview ?? original ?? undefined}
                alt={photo.filename}
                className="viewer-img max-h-[calc(100vh-220px)]"
                draggable={false}
                onMouseDown={(e) => {
                  if (!cropMode) return;
                  const c = toImgCoords(e);
                  if (!c) return;
                  setCropDrag({ x0: c.x, y0: c.y, x1: c.x, y1: c.y });
                }}
                onMouseMove={(e) => {
                  if (!cropMode || !cropDrag) return;
                  const c = toImgCoords(e);
                  if (!c) return;
                  setCropDrag({ ...cropDrag, x1: c.x, y1: c.y });
                }}
                onMouseUp={(e) => {
                  if (!cropMode || !cropDrag) return;
                  const c = toImgCoords(e);
                  if (!c) return;
                  const crop = displayRectToCrop(
                    Math.min(cropDrag.x0, c.x),
                    Math.min(cropDrag.y0, c.y),
                    Math.abs(c.x - cropDrag.x0),
                    Math.abs(c.y - cropDrag.y0),
                    c.w,
                    c.h,
                  );
                  setCropDrag(null);
                  setCropMode(false);
                  if (crop) update({ ...paramsRef.current, crop });
                }}
              />
              {rect && rect.w > 4 && rect.h > 4 && <div className="crop-rect" style={rect} />}
            </div>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-3 border-t border-neutral-800 px-3 py-2 text-xs text-neutral-400">
          <span>{showOriginal ? "Original" : cropMode ? "Drag on the image to select a crop area" : "Edited preview (non-destructive)"}</span>
          {busy && <span className="text-neutral-300">{busy}</span>}
          {exportMsg && <span className="text-emerald-400">{exportMsg}</span>}
        </div>
      </div>

      {/* controls */}
      <div className="w-72 shrink-0 overflow-y-auto border-l border-neutral-800 p-4">
        <div className="mb-4 flex gap-2">
          <button
            className="flex-1 rounded-lg bg-neutral-800 py-1.5 text-xs hover:bg-neutral-700"
            onClick={() => update(rotate90(paramsRef.current, -1))}
          >
            ⟲ 90°
          </button>
          <button
            className="flex-1 rounded-lg bg-neutral-800 py-1.5 text-xs hover:bg-neutral-700"
            onClick={() => update(rotate90(paramsRef.current, 1))}
          >
            ⟳ 90°
          </button>
          <button
            className="flex-1 rounded-lg bg-neutral-800 py-1.5 text-xs hover:bg-neutral-700"
            onClick={() => update({ ...paramsRef.current, flip_h: !paramsRef.current.flip_h })}
          >
            Flip H
          </button>
          <button
            className="flex-1 rounded-lg bg-neutral-800 py-1.5 text-xs hover:bg-neutral-700"
            onClick={() => update({ ...paramsRef.current, flip_v: !paramsRef.current.flip_v })}
          >
            Flip V
          </button>
        </div>

        {params.crop && (
          <div className="mb-4 flex items-center justify-between rounded-lg bg-neutral-800 px-2.5 py-1.5 text-xs">
            <span>Crop applied</span>
            <button className="text-red-400 hover:underline" onClick={() => update({ ...paramsRef.current, crop: null })}>
              remove
            </button>
          </div>
        )}

        <div className="space-y-4">
          <Slider label="Exposure" value={params.exposure} min={-3} max={3} step={0.05} format={(v) => `${v > 0 ? "+" : ""}${v.toFixed(2)} ev`} onChange={(v) => update({ ...params, exposure: v })} />
          <Slider label="Contrast" value={params.contrast} min={-100} max={100} step={1} onChange={(v) => update({ ...params, contrast: v })} />
          <Slider label="Brightness" value={params.brightness} min={-100} max={100} step={1} onChange={(v) => update({ ...params, brightness: v })} />
          <Slider label="Saturation" value={params.saturation} min={-100} max={100} step={1} onChange={(v) => update({ ...params, saturation: v })} />
          <Slider label="Sharpen" value={params.sharpen} min={0} max={100} step={1} onChange={(v) => update({ ...params, sharpen: v })} />
        </div>

        <button
          onClick={doExport}
          className="mt-6 w-full rounded-lg bg-brand-600 py-2 text-sm font-semibold text-white hover:bg-brand-500"
        >
          Export edited copy…
        </button>
        <p className="mt-2 text-[11px] leading-snug text-neutral-500">
          Edits are non-destructive: the original file is never changed. Export writes a new file.
        </p>
      </div>
    </div>
  );
}
