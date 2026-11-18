import { useEffect, useMemo, useState } from "react";
import { useApp } from "../store";
import { api } from "../lib/api";
import { getThumbUrl } from "../components/Gallery";
import type { Photo } from "../lib/types";
import { formatDateGroup } from "../lib/format";

function Thumb({ photo, onOpen }: { photo: Photo; onOpen: () => void }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let c = false;
    getThumbUrl(photo.id, 256).then((u) => !c && setUrl(u));
    return () => {
      c = true;
    };
  }, [photo.id]);
  return (
    <div
      className="group relative h-32 cursor-pointer overflow-hidden rounded-lg bg-neutral-800"
      onClick={onOpen}
      title={photo.filename}
    >
      {url && <img src={url} className="thumb-img" draggable={false} loading="lazy" />}
      <div className="pointer-events-none absolute inset-x-0 bottom-0 truncate bg-gradient-to-t from-black/80 to-transparent px-2 pb-1 pt-3 text-[10px] text-neutral-200 opacity-0 group-hover:opacity-100">
        {photo.filename}
      </div>
    </div>
  );
}

export default function TimelineView() {
  const { openViewer } = useApp();
  const [photos, setPhotos] = useState<Photo[]>([]);

  useEffect(() => {
    api
      .listPhotos({ search: "", sort: "taken_desc", favorites_only: false, album_id: null, limit: 5000, offset: 0 })
      .then((r) => setPhotos(r.photos))
      .catch(() => {});
  }, []);

  const groups = useMemo(() => {
    const map = new Map<string, Photo[]>();
    for (const p of photos) {
      const key = formatDateGroup(p.taken_at);
      const arr = map.get(key) ?? [];
      arr.push(p);
      map.set(key, arr);
    }
    return [...map.entries()];
  }, [photos]);

  if (photos.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-neutral-500">
        No photos yet — import a folder from the Library.
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      {groups.map(([date, ps]) => (
        <section key={date} className="mb-4">
          <h2 className="sticky top-0 z-10 border-b border-neutral-800 bg-neutral-950/95 px-4 py-2 text-sm font-semibold">
            {date} <span className="ml-1 text-xs font-normal text-neutral-500">{ps.length}</span>
          </h2>
          <div className="grid grid-cols-2 gap-2 p-3 sm:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8">
            {ps.map((p) => (
              <Thumb key={p.id} photo={p} onOpen={() => openViewer(p.id)} />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
