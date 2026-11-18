import { useCallback, useEffect, useState } from "react";
import { useApp } from "../store";
import { api, imageUrl } from "../lib/api";
import type { Photo } from "../lib/types";
import { formatBytes, formatDateTime } from "../lib/format";

export default function Viewer() {
  const { photos, selectedId, closeViewer, setRoute, openViewer, albums, refreshAlbums } = useApp();
  const [photo, setPhoto] = useState<Photo | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [showInfo, setShowInfo] = useState(false);
  const [fav, setFav] = useState(false);

  const index = selectedId !== null ? photos.findIndex((p) => p.id === selectedId) : -1;

  const load = useCallback(
    async (id: number) => {
      setErr(null);
      setUrl(null);
      try {
        const p = await api.getPhoto(id);
        setPhoto(p);
        setFav(p.favorite);
        const u = await imageUrl(id);
        setUrl(u);
      } catch (e) {
        setErr(String(e));
      }
    },
    [],
  );

  useEffect(() => {
    if (selectedId !== null) void load(selectedId);
  }, [selectedId, load]);

  const step = useCallback(
    (dir: number) => {
      if (index < 0) return;
      const next = (index + dir + photos.length) % photos.length;
      openViewer(photos[next].id);
    },
    [index, photos, openViewer],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeViewer();
      else if (e.key === "ArrowRight") step(1);
      else if (e.key === "ArrowLeft") step(-1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closeViewer, step]);

  if (!photo) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/90 text-neutral-400">
        {err ?? "Loading…"}
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-black/95">
      {/* top bar */}
      <div className="flex h-12 shrink-0 items-center gap-2 px-4 text-sm text-neutral-300">
        <span className="max-w-md truncate font-medium">{photo.filename}</span>
        <span className="text-xs text-neutral-500">{formatBytes(photo.size_bytes)}</span>
        <div className="ml-auto flex items-center gap-2">
          <button
            className={`rounded-lg px-2.5 py-1.5 hover:bg-neutral-800 ${fav ? "text-amber-400" : ""}`}
            title="Favorite"
            onClick={async () => {
              const next = !fav;
              setFav(next);
              await api.setFavorite(photo.id, next);
            }}
          >
            <svg viewBox="0 0 24 24" className="h-5 w-5" fill={fav ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round">
              <path d="M12 2.5l2.9 6 6.6.9-4.8 4.6 1.2 6.5L12 17.4l-5.9 3.1 1.2-6.5L2.5 9.4l6.6-.9z" />
            </svg>
          </button>
          <button
            className="rounded-lg px-2.5 py-1.5 hover:bg-neutral-800"
            title="Photo info"
            onClick={() => setShowInfo((v) => !v)}
          >
            Info
          </button>
          <button
            className="rounded-lg px-2.5 py-1.5 hover:bg-neutral-800"
            onClick={() => {
              setRoute("editor");
              closeViewer();
            }}
          >
            Edit
          </button>
          <button
            className="rounded-lg px-2.5 py-1.5 hover:bg-neutral-800"
            onClick={() => {
              setRoute("enhance");
              closeViewer();
            }}
          >
            AI Enhance
          </button>
          <select
            className="rounded-lg border border-neutral-700 bg-neutral-900 px-2 py-1.5 text-xs"
            value=""
            onChange={async (e) => {
              const albumId = Number(e.target.value);
              if (albumId > 0) {
                await api.addToAlbum(albumId, photo.id);
                await refreshAlbums();
              }
            }}
          >
            <option value="">+ Album…</option>
            {albums.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
          <button className="rounded-lg px-2.5 py-1.5 text-lg leading-none hover:bg-neutral-800" onClick={closeViewer} title="Close (Esc)">
            ×
          </button>
        </div>
      </div>

      {/* image area */}
      <div className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden">
        {url ? (
          <img src={url} alt={photo.filename} className="viewer-img" draggable={false} />
        ) : (
          <div className="text-sm text-neutral-500">{err ?? "Loading image…"}</div>
        )}
        {photos.length > 1 && (
          <>
            <button
              className="absolute left-3 top-1/2 -translate-y-1/2 rounded-full bg-black/50 px-3 py-2 text-xl hover:bg-black/70"
              onClick={() => step(-1)}
            >
              ‹
            </button>
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 rounded-full bg-black/50 px-3 py-2 text-xl hover:bg-black/70"
              onClick={() => step(1)}
            >
              ›
            </button>
          </>
        )}

        {showInfo && (
          <div className="absolute right-3 top-3 w-72 rounded-xl border border-neutral-700 bg-neutral-900/95 p-4 text-xs text-neutral-300 shadow-xl">
            <div className="pb-2 text-sm font-semibold text-neutral-100">Photo info</div>
            <dl className="space-y-1">
              <Row k="File" v={photo.filename} />
              <Row k="Path" v={photo.path} />
              <Row k="Size" v={formatBytes(photo.size_bytes)} />
              <Row
                k="Dimensions"
                v={photo.width && photo.height ? `${photo.width} × ${photo.height}` : "—"}
              />
              <Row k="Taken" v={formatDateTime(photo.taken_at)} />
              <Row k="Camera" v={photo.camera} />
              <Row k="Lens" v={photo.lens} />
              <Row k="ISO" v={photo.iso} />
              <Row k="Aperture" v={photo.aperture} />
              <Row k="Focal" v={photo.focal_length} />
              <Row k="Exposure" v={photo.exposure_time} />
              <Row
                k="GPS"
                v={
                  photo.gps_lat !== null && photo.gps_lon !== null
                    ? `${photo.gps_lat.toFixed(6)}, ${photo.gps_lon.toFixed(6)}`
                    : null
                }
              />
              <Row k="SHA-256" v={photo.sha256 ? photo.sha256.slice(0, 16) + "…" : null} />
            </dl>
          </div>
        )}
      </div>

      {/* bottom strip */}
      <div className="flex h-8 shrink-0 items-center justify-center gap-4 text-[11px] text-neutral-500">
        <span>
          {index + 1} / {photos.length}
        </span>
        <span>{photo.path}</span>
      </div>
    </div>
  );
}

function Row({ k, v }: { k: string; v: string | null }) {
  if (!v) return null;
  return (
    <div className="flex gap-2">
      <dt className="w-20 shrink-0 text-neutral-500">{k}</dt>
      <dd className="min-w-0 break-all">{v}</dd>
    </div>
  );
}
