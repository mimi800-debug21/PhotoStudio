import { useEffect, useMemo, useRef, useState } from "react";
import { FixedSizeGrid, type GridChildComponentProps } from "react-window";
import { useApp } from "../store";
import { api } from "../lib/api";
import type { Photo } from "../lib/types";

const CELL = 190;
const GAP = 6;

const thumbUrlCache = new Map<string, string>();

export async function getThumbUrl(id: number, size: number): Promise<string> {
  const key = `${id}:${size}`;
  const cached = thumbUrlCache.get(key);
  if (cached) return cached;
  const bytes = await api.getThumbnail(id, size);
  const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/jpeg" }));
  thumbUrlCache.set(key, url);
  return url;
}

export function invalidateThumb(id: number) {
  for (const key of [...thumbUrlCache.keys()]) {
    if (key.startsWith(`${id}:`)) thumbUrlCache.delete(key);
  }
}

function ThumbCellInner({ photo, onOpen }: { photo: Photo; onOpen: (id: number) => void }) {
  const [url, setUrl] = useState<string | null>(null);
  const [err, setErr] = useState(false);
  const [fav, setFav] = useState(photo.favorite);

  useEffect(() => {
    let cancelled = false;
    setErr(false);
    getThumbUrl(photo.id, 320)
      .then((u) => !cancelled && setUrl(u))
      .catch(() => !cancelled && setErr(true));
    return () => {
      cancelled = true;
    };
  }, [photo.id]);

  return (
    <div
      className="group relative cursor-pointer overflow-hidden rounded-lg bg-neutral-800"
      style={{ width: CELL - GAP, height: CELL - GAP }}
      onClick={() => onOpen(photo.id)}
      title={photo.path}
    >
      {url ? (
        <img src={url} alt={photo.filename} className="thumb-img" loading="lazy" draggable={false} />
      ) : err ? (
        <div className="flex h-full w-full items-center justify-center text-xs text-neutral-500">no preview</div>
      ) : (
        <div className="h-full w-full animate-pulse bg-neutral-800" />
      )}
      <button
        className={`absolute right-1.5 top-1.5 rounded-full bg-black/50 p-1 opacity-0 transition-opacity group-hover:opacity-100 ${
          fav ? "text-amber-400" : "text-neutral-300 hover:text-amber-300"
        }`}
        onClick={async (e) => {
          e.stopPropagation();
          const next = !fav;
          setFav(next);
          try {
            await api.setFavorite(photo.id, next);
          } catch {
            setFav(!next);
          }
        }}
        title="Favorite"
      >
        <svg viewBox="0 0 24 24" className="h-4 w-4" fill={fav ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round">
          <path d="M12 2.5l2.9 6 6.6.9-4.8 4.6 1.2 6.5L12 17.4l-5.9 3.1 1.2-6.5L2.5 9.4l6.6-.9z" />
        </svg>
      </button>
      <div className="pointer-events-none absolute inset-x-0 bottom-0 truncate bg-gradient-to-t from-black/80 to-transparent px-2 pb-1 pt-4 text-[11px] text-neutral-200 opacity-0 transition-opacity group-hover:opacity-100">
        {photo.filename}
      </div>
      {photo.format.startsWith("raw:") && (
        <span className="absolute left-1.5 top-1.5 rounded bg-black/60 px-1 py-0.5 text-[9px] font-bold uppercase text-neutral-300">
          {photo.format.replace("raw:", "")}
        </span>
      )}
    </div>
  );
}

export default function Gallery({ photos, onOpen }: { photos: Photo[]; onOpen: (id: number) => void }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [height, setHeight] = useState(600);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0].contentRect;
      setWidth(r.width);
      setHeight(r.height);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const columnCount = Math.max(1, Math.floor((width + GAP) / (CELL + GAP)));
  const rowCount = Math.ceil(photos.length / columnCount);

  const Cell = useMemo(
    () =>
      ({ columnIndex, rowIndex, style }: GridChildComponentProps) => {
        const idx = rowIndex * columnCount + columnIndex;
        const photo = photos[idx];
        if (!photo) return <div style={style} />;
        return (
          <div style={{ ...style, padding: GAP / 2 }}>
            <ThumbCellInner photo={photo} onOpen={onOpen} />
          </div>
        );
      },
    [photos, columnCount, onOpen],
  );

  if (photos.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="max-w-sm text-center text-sm text-neutral-500">
          No photos here yet.
          <br />
          Use <span className="font-semibold text-neutral-300">Import folder</span> to add your photos. Originals are
          never modified.
        </div>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="h-full w-full">
      <FixedSizeGrid
        columnCount={columnCount}
        columnWidth={CELL + GAP}
        height={height}
        rowCount={rowCount}
        rowHeight={CELL + GAP}
        width={width}
        overscanRowCount={3}
      >
        {Cell}
      </FixedSizeGrid>
    </div>
  );
}
