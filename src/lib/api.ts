import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AiProgress,
  AiStatus,
  Album,
  AppInfo,
  DuplicateGroup,
  EditParams,
  EnhanceResult,
  ExportRecord,
  ImportResult,
  ImportedFolder,
  Photo,
  PhotoListResult,
  PhotoQuery,
} from "./types";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),

  pickFolder: () => invoke<string | null>("pick_folder"),
  pickSaveFile: (suggestedName: string, defaultDir?: string) =>
    invoke<string | null>("pick_save_file", { suggestedName, defaultDir }),

  importFolder: (path: string, recursive: boolean) =>
    invoke<ImportResult>("import_folder", { path, recursive }),
  listFolders: () => invoke<ImportedFolder[]>("list_imported_folders"),
  removeFolder: (id: number) => invoke<void>("remove_imported_folder", { id }),
  rescanFolder: (id: number) => invoke<ImportResult>("rescan_folder", { id }),

  listPhotos: (query: PhotoQuery) => invoke<PhotoListResult>("list_photos", { query }),
  getPhoto: (id: number) => invoke<Photo>("get_photo", { id }),
  setFavorite: (id: number, favorite: boolean) =>
    invoke<void>("set_favorite", { id, favorite }),
  findDuplicates: () => invoke<DuplicateGroup[]>("find_duplicates"),

  getThumbnail: (id: number, size: number) =>
    invoke<number[]>("get_thumbnail", { id, size }),
  getImageBytes: (id: number) => invoke<number[]>("get_image_bytes", { id }),

  getEdits: (id: number) => invoke<EditParams | null>("get_edits", { id }),
  saveEdits: (id: number, params: EditParams) =>
    invoke<void>("save_edits", { id, params }),
  renderPreview: (id: number, params: EditParams, maxDim: number) =>
    invoke<number[]>("render_preview", { id, params, maxDim }),
  exportPhoto: (id: number, params: EditParams, outPath: string) =>
    invoke<ExportRecord>("export_photo", { id, params, outPath }),
  listExports: (id: number) => invoke<ExportRecord[]>("list_exports", { id }),

  listAlbums: () => invoke<Album[]>("list_albums"),
  createAlbum: (name: string) => invoke<Album>("create_album", { name }),
  deleteAlbum: (id: number) => invoke<void>("delete_album", { id }),
  addToAlbum: (albumId: number, photoId: number) =>
    invoke<void>("add_to_album", { albumId, photoId }),
  removeFromAlbum: (albumId: number, photoId: number) =>
    invoke<void>("remove_from_album", { albumId, photoId }),
  albumContains: (albumId: number, photoId: number) =>
    invoke<boolean>("album_contains", { albumId, photoId }),

  aiStatus: () => invoke<AiStatus>("ai_status"),
  aiStartSetup: () => invoke<void>("ai_start_setup"),
  aiCancelSetup: () => invoke<void>("ai_cancel_setup"),
  aiSelfTest: () => invoke<AiStatus>("ai_self_test"),
  aiEnhance: (params: {
    photoId: number;
    model: string;
    scale: number;
    tile: number;
    fp16: boolean;
    outPath: string;
  }) => invoke<string>("ai_enhance", { params }),
  aiCancelJob: (jobId: string) => invoke<void>("ai_cancel_job", { jobId }),
  aiOpenRuntimeDir: () => invoke<void>("ai_open_runtime_dir"),
  readExportBytes: (path: string) => invoke<number[]>("read_export_bytes", { path }),

  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  backupDatabase: () => invoke<string>("backup_database"),
};

export interface AiEventPayload {
  kind: string;
  progress?: AiProgress;
  status?: AiStatus;
  message?: string;
  job_id?: string;
  result?: EnhanceResult;
  error?: string;
}

export function onAiEvent(cb: (evt: AiEventPayload) => void): Promise<UnlistenFn> {
  return listen<AiEventPayload>("ai-event", (event) => cb(event.payload));
}

/** Fetch a thumbnail as an object URL. */
export async function thumbUrl(id: number, size = 320): Promise<string> {
  const bytes = await api.getThumbnail(id, size);
  const blob = new Blob([new Uint8Array(bytes)], { type: "image/jpeg" });
  return URL.createObjectURL(blob);
}

export async function imageUrl(id: number): Promise<string> {
  const bytes = await api.getImageBytes(id);
  const blob = new Blob([new Uint8Array(bytes)]);
  return URL.createObjectURL(blob);
}

export async function previewUrl(id: number, params: EditParams, maxDim = 1600): Promise<string> {
  const bytes = await api.renderPreview(id, params, maxDim);
  const blob = new Blob([new Uint8Array(bytes)], { type: "image/jpeg" });
  return URL.createObjectURL(blob);
}

export type { Photo, PhotoQuery };
