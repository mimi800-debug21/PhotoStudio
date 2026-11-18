import { useEffect, useState } from "react";
import { useApp } from "../store";
import { api, imageUrl } from "../lib/api";
import type { EnhanceResult, Photo } from "../lib/types";
import { formatBytes, formatDuration } from "../lib/format";

const MODELS = [
  { id: "realesrgan_x4plus", label: "Real-ESRGAN x4plus", scale: 4 },
  { id: "realesrgan_x2plus", label: "Real-ESRGAN x2plus", scale: 2 },
] as const;

export default function EnhanceView() {
  const { selectedId, setRoute, aiStatus, aiProgress, refreshAiStatus, jobResult, jobError } = useApp();
  const [photo, setPhoto] = useState<Photo | null>(null);
  const [model, setModel] = useState<(typeof MODELS)[number]>(MODELS[0]);
  const [tile, setTile] = useState<string>("auto");
  const [fp16, setFp16] = useState(false);
  const [outPath, setOutPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [result, setResult] = useState<EnhanceResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [beforeUrl, setBeforeUrl] = useState<string | null>(null);
  const [afterUrl, setAfterUrl] = useState<string | null>(null);

  useEffect(() => {
    void refreshAiStatus();
  }, [refreshAiStatus]);

  useEffect(() => {
    if (selectedId === null) return;
    let cancelled = false;
    api.getPhoto(selectedId).then((p) => !cancelled && setPhoto(p)).catch(() => {});
    imageUrl(selectedId)
      .then((u) => !cancelled && setBeforeUrl(u))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const ready = aiStatus?.state === "ready";

  const chooseOutput = async () => {
    if (!photo) return;
    const base = photo.filename.replace(/\.[^.]+$/, "");
    const out = await api.pickSaveFile(`${base}_x${model.scale}.jpg`);
    if (out) setOutPath(out);
  };

  // react to job completion events
  useEffect(() => {
    if (jobResult && jobId && jobResult.jobId === jobId) {
      setResult(jobResult.result);
      setBusy(false);
      setJobId(null);
      void api
        .readExportBytes(jobResult.result.output)
        .then((bytes) =>
          setAfterUrl(URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/jpeg" }))),
        )
        .catch((e) => setError(String(e)));
      void refreshAiStatus();
    }
    if (jobError && jobId && jobError.jobId === jobId) {
      setError(jobError.error);
      setBusy(false);
      setJobId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobResult, jobError]);

  const start = async () => {
    if (!photo || !outPath) return;
    setBusy(true);
    setError(null);
    setResult(null);
    setAfterUrl(null);
    try {
      const tileNum = tile === "auto" ? -1 : Number(tile);
      const id = await api.aiEnhance({
        photoId: photo.id,
        model: model.id,
        scale: model.scale,
        tile: tileNum,
        fp16,
        outPath,
      });
      setJobId(id);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  if (selectedId === null || !photo) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-neutral-500">
        {selectedId === null ? (
          <>
            Open a photo from the{" "}
            <button className="text-brand-400 underline" onClick={() => setRoute("library")}>
              Library
            </button>{" "}
            to enhance it.
          </>
        ) : (
          "Loading…"
        )}
      </div>
    );
  }

  const progress = jobId && aiProgress?.job_id === jobId ? aiProgress : aiProgress;

  return (
    <div className="flex h-full">
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 items-center gap-2 border-b border-neutral-800 px-3 py-1.5 text-xs">
          <span className="max-w-xs truncate font-medium">{photo.filename}</span>
          <span
            className={`rounded-full px-2 py-0.5 font-semibold ${
              ready
                ? aiStatus?.mode === "cuda"
                  ? "bg-emerald-500/20 text-emerald-300"
                  : "bg-amber-500/20 text-amber-300"
                : "bg-neutral-700 text-neutral-300"
            }`}
          >
            {ready ? (aiStatus?.mode === "cuda" ? `GPU: ${aiStatus.device_name ?? "CUDA"}` : "CPU mode") : "AI not set up"}
          </span>
        </div>
        <div className="flex min-h-0 flex-1 items-center justify-center gap-6 overflow-auto p-6">
          <div className="text-center">
            <div className="mb-1 text-xs text-neutral-500">Before</div>
            {beforeUrl ? (
              <img src={beforeUrl} className="viewer-img max-h-[60vh] rounded-lg border border-neutral-800" draggable={false} />
            ) : (
              <div className="h-48 w-64 animate-pulse rounded-lg bg-neutral-800" />
            )}
            <div className="mt-1 text-[11px] text-neutral-500">{photo.width && photo.height ? `${photo.width}×${photo.height}` : ""}</div>
          </div>
          <div className="text-2xl text-neutral-600">→</div>
          <div className="text-center">
            <div className="mb-1 text-xs text-neutral-500">After</div>
            {afterUrl ? (
              <img src={afterUrl} className="viewer-img max-h-[60vh] rounded-lg border border-brand-600/50" draggable={false} />
            ) : (
              <div className="flex h-48 w-64 items-center justify-center rounded-lg bg-neutral-800 text-xs text-neutral-500">
                {busy ? "Processing…" : "Result appears here"}
              </div>
            )}
            {result && (
              <div className="mt-1 text-[11px] text-neutral-500">
                {result.scale}× · {result.model} · {result.device} · {formatDuration(result.elapsed_ms)}
              </div>
            )}
          </div>
        </div>
        {busy && progress && (
          <div className="shrink-0 border-t border-neutral-800 px-4 py-3">
            <div className="mb-1 flex justify-between text-xs text-neutral-400">
              <span>{progress.message || progress.phase}</span>
              <span>{progress.percent.toFixed(0)}%</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-neutral-800">
              <div className="h-full bg-brand-500 transition-all" style={{ width: `${progress.percent}%` }} />
            </div>
          </div>
        )}
        {error && <div className="shrink-0 px-4 py-2 text-xs text-red-400">{error}</div>}
        {result && !busy && (
          <div className="shrink-0 px-4 py-2 text-xs text-emerald-400">Enhanced image saved to {result.output}</div>
        )}
      </div>

      <div className="w-72 shrink-0 overflow-y-auto border-l border-neutral-800 p-4">
        {!ready && (
          <div className="mb-4 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-200">
            The AI runtime is not installed yet. Go to{" "}
            <button className="underline" onClick={() => setRoute("settings")}>
              Settings → AI Setup
            </button>{" "}
            and run the first-run setup.
          </div>
        )}
        <div className="mb-2 text-xs font-semibold uppercase tracking-wider text-neutral-500">Model</div>
        <div className="mb-4 space-y-1.5">
          {MODELS.map((m) => (
            <label
              key={m.id}
              className={`flex cursor-pointer items-center gap-2 rounded-lg border px-2.5 py-2 text-sm ${
                model.id === m.id ? "border-brand-500 bg-brand-600/15" : "border-neutral-700 hover:bg-neutral-800"
              }`}
            >
              <input
                type="radio"
                name="model"
                checked={model.id === m.id}
                onChange={() => setModel(m)}
                className="accent-brand-500"
              />
              {m.label} <span className="ml-auto text-xs text-neutral-500">{m.scale}×</span>
            </label>
          ))}
        </div>

        <div className="mb-2 text-xs font-semibold uppercase tracking-wider text-neutral-500">Tile size</div>
        <select
          value={tile}
          onChange={(e) => setTile(e.target.value)}
          className="mb-4 w-full rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5 text-sm"
        >
          <option value="auto">Auto (memory-aware)</option>
          <option value="128">128 px</option>
          <option value="256">256 px</option>
          <option value="512">512 px</option>
          <option value="0">No tiling</option>
        </select>

        <label className="mb-4 flex items-center gap-2 text-sm text-neutral-300">
          <input type="checkbox" checked={fp16} onChange={(e) => setFp16(e.target.checked)} className="accent-brand-500" />
          FP16 (experimental)
        </label>

        <div className="mb-2 text-xs font-semibold uppercase tracking-wider text-neutral-500">Output</div>
        <button
          onClick={chooseOutput}
          className="mb-2 w-full rounded-lg bg-neutral-800 py-2 text-sm hover:bg-neutral-700"
        >
          {outPath ? "Change output file…" : "Choose output file…"}
        </button>
        {outPath && <div className="mb-4 break-all text-[11px] text-neutral-500">{outPath}</div>}

        <div className="flex gap-2">
          <button
            onClick={start}
            disabled={!ready || busy || !outPath}
            className="flex-1 rounded-lg bg-brand-600 py-2 text-sm font-semibold text-white hover:bg-brand-500 disabled:opacity-40"
          >
            {busy ? "Enhancing…" : `Enhance ${model.scale}×`}
          </button>
          {busy && (
            <button
              onClick={() => {
                if (jobId) void api.aiCancelJob(jobId);
              }}
              className="rounded-lg bg-neutral-800 px-3 py-2 text-sm hover:bg-neutral-700"
            >
              Cancel
            </button>
          )}
        </div>
        <p className="mt-3 text-[11px] leading-snug text-neutral-500">
          Real-ESRGAN runs locally on your machine. The original photo is never modified; the result is written to the
          output file you choose. {aiStatus?.models ? `${aiStatus.models.filter((m) => m.present).length}/${aiStatus.models.length} models downloaded.` : ""}
        </p>
      </div>
    </div>
  );
}
