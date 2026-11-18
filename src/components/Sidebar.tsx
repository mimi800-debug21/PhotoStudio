import { useEffect } from "react";
import { useApp, type Route } from "../store";
import { api } from "../lib/api";

const NAV: { id: Route; label: string; icon: string }[] = [
  { id: "library", label: "Library", icon: "M4 5h16v14H4z M4 15l4-4 4 4 3-3 5 5" },
  { id: "editor", label: "Editor", icon: "M12 3l9 5-9 5-9-5 9-5z M3 13l9 5 9-5" },
  { id: "enhance", label: "AI Enhance", icon: "M12 2v4 M12 18v4 M2 12h4 M18 12h4 M5 5l2.5 2.5 M16.5 16.5L19 19 M19 5l-2.5 2.5 M7.5 16.5L5 19" },
  { id: "albums", label: "Albums", icon: "M3 6a2 2 0 012-2h4l2 2h8a2 2 0 012 2v10a2 2 0 01-2 2H5a2 2 0 01-2-2V6z" },
  { id: "timeline", label: "Timeline", icon: "M4 6h16 M4 12h16 M4 18h10" },
  { id: "settings", label: "Settings", icon: "M12 15a3 3 0 100-6 3 3 0 000 6z M19 12a7 7 0 01-.1 1.2l2 1.6-2 3.4-2.4-1a7 7 0 01-2 1.2L14 21h-4l-.5-2.6a7 7 0 01-2-1.2l-2.4 1-2-3.4 2-1.6A7 7 0 015 12a7 7 0 01.1-1.2l-2-1.6 2-3.4 2.4 1a7 7 0 012-1.2L10 3h4l.5 2.6a7 7 0 012 1.2l2.4-1 2 3.4-2 1.6c.06.4.1.8.1 1.2z" },
];

export default function Sidebar() {
  const { route, setRoute, folders, refreshFolders, setAlbumFilter, aiStatus, theme, toggleTheme } = useApp();

  useEffect(() => {
    void refreshFolders();
  }, [refreshFolders]);

  const aiBadge =
    aiStatus?.state === "ready"
      ? aiStatus.mode === "cuda"
        ? "GPU"
        : "CPU"
      : aiStatus?.state === "installing"
        ? "…"
        : "AI";

  return (
    <aside className="flex h-full w-56 shrink-0 flex-col border-r border-neutral-800 bg-neutral-900/60">
      <div className="flex items-center gap-2 px-4 pb-3 pt-4">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-brand-400 to-violet-500 text-sm font-bold text-white">
          PS
        </div>
        <div className="text-lg font-semibold tracking-tight">PhotoStudio</div>
      </div>

      <nav className="flex flex-col gap-0.5 px-2">
        {NAV.map((n) => (
          <button
            key={n.id}
            onClick={() => {
              setRoute(n.id);
              if (n.id === "library") setAlbumFilter(null);
            }}
            className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
              route === n.id
                ? "bg-brand-600/20 text-brand-300"
                : "text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            }`}
          >
            <svg viewBox="0 0 24 24" className="h-4 w-4 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <path d={n.icon} />
            </svg>
            <span>{n.label}</span>
            {n.id === "enhance" && (
              <span
                className={`ml-auto rounded-full px-1.5 py-0.5 text-[10px] font-semibold ${
                  aiStatus?.state === "ready"
                    ? aiStatus.mode === "cuda"
                      ? "bg-emerald-500/20 text-emerald-300"
                      : "bg-amber-500/20 text-amber-300"
                    : "bg-neutral-700 text-neutral-300"
                }`}
              >
                {aiBadge}
              </span>
            )}
          </button>
        ))}
      </nav>

      <div className="mt-4 flex-1 overflow-y-auto px-2">
        <div className="px-3 pb-1 text-[11px] font-semibold uppercase tracking-wider text-neutral-500">
          Folders
        </div>
        {folders.length === 0 && (
          <div className="px-3 py-1 text-xs text-neutral-500">No folders imported yet.</div>
        )}
        {folders.map((f) => (
          <div
            key={f.id}
            className="group flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs text-neutral-400 hover:bg-neutral-800"
            title={f.path}
          >
            <svg viewBox="0 0 24 24" className="h-3.5 w-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.6">
              <path d="M3 6a2 2 0 012-2h4l2 2h8a2 2 0 012 2v10a2 2 0 01-2 2H5a2 2 0 01-2-2V6z" />
            </svg>
            <span className="truncate">{f.path.split(/[\\/]/).filter(Boolean).pop() ?? f.path}</span>
            <button
              className="ml-auto hidden rounded px-1 text-neutral-500 hover:text-red-400 group-hover:block"
              title="Remove folder from library (files are not touched)"
              onClick={async () => {
                await api.removeFolder(f.id);
                await refreshFolders();
              }}
            >
              ×
            </button>
          </div>
        ))}
      </div>

      <div className="border-t border-neutral-800 p-2">
        <button
          onClick={toggleTheme}
          className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
        >
          {theme === "dark" ? (
            <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
              <circle cx="12" cy="12" r="4" />
              <path d="M12 2v2 M12 20v2 M4.9 4.9l1.4 1.4 M17.7 17.7l1.4 1.4 M2 12h2 M20 12h2 M4.9 19.1l1.4-1.4 M17.7 6.3l1.4-1.4" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 12.8A9 9 0 1111.2 3a7 7 0 009.8 9.8z" />
            </svg>
          )}
          {theme === "dark" ? "Dark theme" : "Light theme"}
        </button>
      </div>
    </aside>
  );
}
