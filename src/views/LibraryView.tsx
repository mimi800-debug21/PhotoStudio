import { useState } from "react";
import { useApp } from "../store";
import { api } from "../lib/api";
import Gallery from "../components/Gallery";

export default function LibraryView() {
  const { photos, loading, refreshPhotos, refreshFolders, openViewer, albumFilter, albums, setAlbumFilter } =
    useApp();
  const [busy, setBusy] = useState<string | null>(null);

  const doImport = async () => {
    try {
      const folder = await api.pickFolder();
      if (!folder) return;
      setBusy("Importing photos…");
      const res = await api.importFolder(folder, true);
      setBusy(
        `Imported ${res.added} new photo${res.added === 1 ? "" : "s"} (${res.skipped} already in library)` +
          (res.errors.length ? ` — ${res.errors.length} errors` : ""),
      );
      await refreshPhotos();
      await refreshFolders();
      setTimeout(() => setBusy(null), 6000);
    } catch (e) {
      setBusy(`Import failed: ${e}`);
      setTimeout(() => setBusy(null), 8000);
    }
  };

  const albumName = albumFilter !== null ? albums.find((a) => a.id === albumFilter)?.name : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-neutral-800 px-4 py-2">
        <button
          onClick={doImport}
          className="rounded-lg bg-brand-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-brand-500"
        >
          + Import folder…
        </button>
        {albumFilter !== null && (
          <div className="flex items-center gap-2 rounded-lg bg-neutral-800 px-2.5 py-1.5 text-sm">
            <span>
              Album: <span className="font-semibold">{albumName ?? `#${albumFilter}`}</span>
            </span>
            <button
              className="rounded bg-neutral-700 px-1.5 text-xs hover:bg-neutral-600"
              onClick={() => setAlbumFilter(null)}
            >
              show all
            </button>
          </div>
        )}
        <div className="ml-auto text-xs text-neutral-500">
          {busy ?? (loading ? "Loading…" : "Originals are never modified")}
        </div>
      </div>
      <div className="min-h-0 flex-1 p-2">
        <Gallery photos={photos} onOpen={openViewer} />
      </div>
    </div>
  );
}
