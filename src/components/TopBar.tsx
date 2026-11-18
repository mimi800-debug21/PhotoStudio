import { useEffect, useState } from "react";
import { useApp } from "../store";
import type { SortKey } from "../lib/types";

const SORTS: { id: SortKey; label: string }[] = [
  { id: "taken_desc", label: "Newest first" },
  { id: "taken_asc", label: "Oldest first" },
  { id: "filename_asc", label: "Name A–Z" },
  { id: "filename_desc", label: "Name Z–A" },
  { id: "size_desc", label: "Largest first" },
];

export default function TopBar({ title }: { title: string }) {
  const { route, search, setSearch, sort, setSort, favoritesOnly, setFavoritesOnly, refreshPhotos, total } =
    useApp();
  const [localSearch, setLocalSearch] = useState(search);

  useEffect(() => {
    const t = setTimeout(() => {
      if (localSearch !== search) {
        setSearch(localSearch);
      }
    }, 250);
    return () => clearTimeout(t);
  }, [localSearch, search, setSearch]);

  useEffect(() => {
    if (route === "library") void refreshPhotos();
  }, [search, sort, favoritesOnly, route, refreshPhotos]);

  const showLibraryControls = route === "library";

  return (
    <header className="flex h-14 shrink-0 items-center gap-3 border-b border-neutral-800 px-4">
      <h1 className="w-40 shrink-0 text-base font-semibold tracking-tight">{title}</h1>
      {showLibraryControls && (
        <>
          <div className="relative min-w-0 flex-1">
            <svg
              viewBox="0 0 24 24"
              className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-500"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            >
              <circle cx="11" cy="11" r="7" />
              <path d="M21 21l-4.3-4.3" />
            </svg>
            <input
              value={localSearch}
              onChange={(e) => setLocalSearch(e.target.value)}
              placeholder="Search by filename…"
              className="w-full rounded-lg border border-neutral-700 bg-neutral-800/70 py-1.5 pl-8 pr-3 text-sm outline-none placeholder:text-neutral-500 focus:border-brand-500"
            />
          </div>
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value as SortKey)}
            className="rounded-lg border border-neutral-700 bg-neutral-800/70 px-2 py-1.5 text-sm outline-none focus:border-brand-500"
          >
            {SORTS.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
          <button
            onClick={() => setFavoritesOnly(!favoritesOnly)}
            className={`flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-sm ${
              favoritesOnly
                ? "border-amber-500/50 bg-amber-500/15 text-amber-300"
                : "border-neutral-700 bg-neutral-800/70 text-neutral-300 hover:text-neutral-100"
            }`}
            title="Show favorites only"
          >
            <svg viewBox="0 0 24 24" className="h-4 w-4" fill={favoritesOnly ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round">
              <path d="M12 2.5l2.9 6 6.6.9-4.8 4.6 1.2 6.5L12 17.4l-5.9 3.1 1.2-6.5L2.5 9.4l6.6-.9z" />
            </svg>
            Favorites
          </button>
          <span className="shrink-0 text-xs text-neutral-500">
            {total} {total === 1 ? "photo" : "photos"}
          </span>
        </>
      )}
    </header>
  );
}
