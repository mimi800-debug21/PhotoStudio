export interface Photo {
  id: number;
  path: string;
  filename: string;
  size_bytes: number;
  mtime: string;
  width: number | null;
  height: number | null;
  taken_at: string | null;
  camera: string | null;
  lens: string | null;
  iso: string | null;
  aperture: string | null;
  focal_length: string | null;
  exposure_time: string | null;
  gps_lat: number | null;
  gps_lon: number | null;
  format: string;
  sha256: string | null;
  favorite: boolean;
  added_at: string;
}

export type SortKey = "taken_desc" | "taken_asc" | "filename_asc" | "filename_desc" | "size_desc";

export interface PhotoQuery {
  search: string;
  sort: SortKey;
  favorites_only: boolean;
  album_id: number | null;
  limit: number;
  offset: number;
}

export interface PhotoListResult {
  photos: Photo[];
  total: number;
}

export interface Album {
  id: number;
  name: string;
  created_at: string;
  count: number;
}

export interface ImportedFolder {
  id: number;
  path: string;
  recursive: boolean;
  added_at: string;
}

export interface ImportResult {
  folder: string;
  scanned: number;
  added: number;
  skipped: number;
  errors: string[];
}

export interface CropRect {
  x: number; // relative 0..1
  y: number;
  w: number;
  h: number;
}

export interface EditParams {
  crop: CropRect | null;
  rotate_deg: number; // degrees clockwise
  flip_h: boolean;
  flip_v: boolean;
  exposure: number; // stops, -3..3
  contrast: number; // -100..100
  saturation: number; // -100..100
  brightness: number; // -100..100
  sharpen: number; // 0..100
}

export interface ExportRecord {
  id: number;
  photo_id: number;
  out_path: string;
  kind: string;
  created_at: string;
}

export interface DuplicateGroup {
  sha256: string;
  photos: Photo[];
}

export type AiStepState = "pending" | "running" | "done" | "error" | "skipped";

export interface AiStep {
  name: string;
  state: AiStepState;
  detail: string;
}

export interface AiStatus {
  installed: boolean; // sidecar present + runtime ready
  state: "not_started" | "installing" | "ready" | "error";
  mode: "cuda" | "cpu" | "unknown";
  gpu_available: boolean;
  device_name: string | null;
  cuda_version: string | null;
  torch_version: string | null;
  steps: AiStep[];
  error: string | null;
  runtime_dir: string | null;
  models: { name: string; file: string; present: boolean; verified: boolean }[];
}

export interface AiProgress {
  job_id: string;
  phase: string;
  current: number;
  total: number;
  percent: number;
  message: string;
}

export interface EnhanceResult {
  job_id: string;
  input: string;
  output: string;
  scale: number;
  model: string;
  device: string;
  elapsed_ms: number;
}

export interface AppInfo {
  version: string;
  app_data_dir: string;
  db_path: string;
  thumb_dir: string;
  runtime_dir: string;
  worker_available: boolean;
}

export interface RawDecodeResult {
  width: number;
  height: number;
  jpeg: number[]; // bytes
}
