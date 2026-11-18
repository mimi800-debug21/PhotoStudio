import { useEffect, useState } from "react";
import { useApp } from "../store";
import { api } from "../lib/api";
import { getThumbUrl } from "../components/Gallery";
import type { Photo } from "../lib/types";

function AlbumGrid({ albumId, onChanged }: { albumId: number; onChanged: () => void }) {
  const { openViewer, refreshAlbums } = useApp();
  const [photos, setPhotos] = useState<Photo[]>([]);

  useEffect(() => {
    api
      .listPhotos({ search: "", sort: "taken_desc", favorites_only: false, album_id: albumId, limit: 1000, offset: 0 })
      .then((r) => setPhotos(r.photos))
      .catch(() => {});
  }, [albumId]);

  if (photos.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-neutral-500">
        This album is empty. Open a photo in the Library and use “+ Album…” to add it.
      </div>
    );
  }

  return (
    <div className="grid h-full auto-rows-min content-start grid-cols-2 gap-3 overflow-y-auto p-4 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
      {photos.map((p) => (
        <ThumbWithRemove
          key={p.id}
          photo={p}
          onOpen={() => openViewer(p.id)}
          onRemove={async () => {
            await api.removeFromAlbum(albumId, p.id);
            setPhotos((ps) => ps.filter((x) => x.id !== p.id));
            await refreshAlbums();
            onChanged();
          }}
        />
      ))}
    </div>
  );
}

function ThumbWithRemove({
  photo,
  onOpen,
  onRemove,
}: {
  photo: Photo;
  onOpen: () => void;
  onRemove: () => void;
}) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let c = false;
    getThumbUrl(photo.id, 320).then((u) => !c && setUrl(u));
    return () => {
      c = true;
    };
  }, [photo.id]);
  return (
    <div
      className="group relative h-40 cursor-pointer overflow-hidden rounded-lg bg-neutral-800"
      onClick={onOpen}
      title={photo.filename}
    >
      {url && <img src={url} className="thumb-img" draggable={false} loading="lazy" />}
      <button
        className="absolute right-1.5 top-1.5 hidden rounded-full bg-black/60 px-1.5 text-xs text-neutral-300 hover:text-red-400 group-hover:block"
        title="Remove from album"
        onClick={(e) => {
          e.stopPropagation();
          onRemove();
        }}
      >
        ✕
      </button>
      <div className="pointer-events-none absolute inset-x-0 bottom-0 truncate bg-gradient-to-t from-black/80 to-transparent px-2 pb-1 pt-4 text-[11px] text-neutral-200 opacity-0 group-hover:opacity-100">
        {photo.filename}
      </div>
    </div>
  );
}

export default function AlbumsView() {
  const { albums, refreshAlbums, setAlbumFilter, setRoute } = useApp();
  const [selected, setSelected] = useState<number | null>(null);
  const [newName, setNewName] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    void refreshAlbums();
  }, [refreshAlbums]);

  const create = async () => {
    const name = newName.trim();
    if (!name) return;
    try {
      const a = await api.createAlbum(name);
      setNewName("");
      await refreshAlbums();
      setSelected(a.id);
    } catch (e) {
      setMsg(String(e));
      setTimeout(() => setMsg(null), 5000);
    }
  };

  return (
    <div className="flex h-full">
      <div className="w-64 shrink-0 overflow-y-auto border-r border-neutral-800 p-3">
        <div className="mb-3 flex gap-1.5">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && void create()}
            placeholder="New album name…"
            className="min-w-0 flex-1 rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5 text-sm outline-none focus:border-brand-500"
          />
          <button onClick={create} className="rounded-lg bg-brand-600 px-2.5 text-sm text-white hover:bg-brand-500">
            +
          </button>
        </div>
        {msg && <div className="mb-2 text-xs text-red-400">{msg}</div>}
        <div className="space-y-0.5">
          {albums.length === 0 && <div className="px-1 py-2 text-xs text-neutral-500">No albums yet.</div>}
          {albums.map((a) => (
            <div
              key={a.id}
              className={`group flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-sm ${
                selected === a.id ? "bg-brand-600/20 text-brand-300" : "text-neutral-300 hover:bg-neutral-800"
              }`}
              onClick={() => setSelected(a.id)}
            >
              <span className="min-w-0 flex-1 truncate">{a.name}</span>
              <span className="text-xs text-neutral-500">{a.count}</span>
              <button
                className="hidden rounded px-1 text-neutral-500 hover:text-red-400 group-hover:block"
                title="Delete album (photos are not deleted)"
                onClick={async (e) => {
                  e.stopPropagation();
                  await api.deleteAlbum(a.id);
                  if (selected === a.id) setSelected(null);
                  await refreshAlbums();
                }}
              >
                ✕
              </button>
            </div>
          ))}
        </div>
        {selected !== null && (
          <button
            className="mt-3 w-full rounded-lg bg-neutral-800 py-1.5 text-xs text-neutral-300 hover:bg-neutral-700"
            onClick={() => {
              setAlbumFilter(selected);
              setRoute("library");
            }}
          >
            Show in Library view
          </button>
        )}
      </div>
      <div className="min-w-0 flex-1">
        {selected === null ? (
          <div className="flex h-full items-center justify-center text-sm text-neutral-500">
            Select an album on the left.
          </div>
        ) : (
          <AlbumGrid albumId={selected} onChanged={() => void refreshAlbums()} />
        )}
      </div>
    </div>
  );
}
