import { create } from "zustand";
import { api } from "./lib/api";
import type {
  AiProgress,
  AiStatus,
  Album,
  AppInfo,
  ImportedFolder,
  Photo,
  SortKey,
} from "./lib/types";

export type Route = "library" | "editor" | "enhance" | "albums" | "timeline" | "settings";

interface AppState {
  route: Route;
  setRoute: (r: Route) => void;

  appInfo: AppInfo | null;
  loadAppInfo: () => Promise<void>;

  // library
  photos: Photo[];
  total: number;
  loading: boolean;
  search: string;
  sort: SortKey;
  favoritesOnly: boolean;
  albumFilter: number | null; // when set, library shows an album's photos
  setSearch: (s: string) => void;
  setSort: (s: SortKey) => void;
  setFavoritesOnly: (v: boolean) => void;
  setAlbumFilter: (id: number | null) => void;
  refreshPhotos: () => Promise<void>;

  folders: ImportedFolder[];
  refreshFolders: () => Promise<void>;

  albums: Album[];
  refreshAlbums: () => Promise<void>;

  // selection
  selectedId: number | null;
  selectPhoto: (id: number | null) => void;

  // viewer overlay
  viewerOpen: boolean;
  openViewer: (id: number) => void;
  closeViewer: () => void;

  // theme
  theme: "dark" | "light";
  toggleTheme: () => void;

  // AI
  aiStatus: AiStatus | null;
  aiProgress: AiProgress | null;
  aiBusy: boolean;
  jobResult: { jobId: string; result: import("./lib/types").EnhanceResult } | null;
  jobError: { jobId: string; error: string } | null;
  refreshAiStatus: () => Promise<void>;
  handleAiEvent: (evt: import("./lib/api").AiEventPayload) => void;
}

export const useApp = create<AppState>((set, get) => ({
  route: "library",
  setRoute: (route) => set({ route }),

  appInfo: null,
  loadAppInfo: async () => {
    const appInfo = await api.appInfo();
    set({ appInfo });
  },

  photos: [],
  total: 0,
  loading: false,
  search: "",
  sort: "taken_desc",
  favoritesOnly: false,
  albumFilter: null,
  setSearch: (search) => set({ search }),
  setSort: (sort) => set({ sort }),
  setFavoritesOnly: (favoritesOnly) => set({ favoritesOnly }),
  setAlbumFilter: (albumFilter) => set({ albumFilter }),
  refreshPhotos: async () => {
    const s = get();
    set({ loading: true });
    try {
      const res = await api.listPhotos({
        search: s.search.trim(),
        sort: s.sort,
        favorites_only: s.favoritesOnly,
        album_id: s.albumFilter,
        limit: 500,
        offset: 0,
      });
      set({ photos: res.photos, total: res.total, loading: false });
    } catch (e) {
      console.error("list_photos failed", e);
      set({ loading: false });
    }
  },

  folders: [],
  refreshFolders: async () => {
    try {
      set({ folders: await api.listFolders() });
    } catch (e) {
      console.error(e);
    }
  },

  albums: [],
  refreshAlbums: async () => {
    try {
      set({ albums: await api.listAlbums() });
    } catch (e) {
      console.error(e);
    }
  },

  selectedId: null,
  selectPhoto: (selectedId) => set({ selectedId }),

  viewerOpen: false,
  openViewer: (id) => set({ viewerOpen: true, selectedId: id }),
  closeViewer: () => set({ viewerOpen: false }),

  theme: "dark",
  toggleTheme: () => {
    const t = get().theme === "dark" ? "light" : "dark";
    document.documentElement.classList.toggle("light", t === "light");
    document.documentElement.classList.toggle("dark", t === "dark");
    set({ theme: t });
    void api.setSetting("theme", t).catch(() => {});
  },

  aiStatus: null,
  aiProgress: null,
  aiBusy: false,
  jobResult: null,
  jobError: null,
  refreshAiStatus: async () => {
    try {
      set({ aiStatus: await api.aiStatus() });
    } catch (e) {
      console.error(e);
    }
  },
  handleAiEvent: (evt) => {
    if (evt.kind === "setup-status" && evt.status) {
      set({ aiStatus: evt.status, aiBusy: evt.status.state === "installing" });
    } else if (evt.kind === "progress" && evt.progress) {
      set({ aiProgress: evt.progress });
    } else if (evt.kind === "job-done" && evt.job_id && evt.result) {
      set({ aiBusy: false, jobResult: { jobId: evt.job_id, result: evt.result } });
    } else if (evt.kind === "job-error") {
      set({
        aiBusy: false,
        jobError: { jobId: evt.job_id ?? "?", error: evt.error ?? evt.message ?? "unknown error" },
      });
    }
  },
}));
