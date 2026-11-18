import { useEffect, useState } from "react";
import { useApp } from "../store";
import { api } from "../lib/api";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-6 rounded-xl border border-neutral-800 bg-neutral-900/40 p-4">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wider text-neutral-400">{title}</h2>
      {children}
    </section>
  );
}

export default function SettingsView() {
  const { appInfo, aiStatus, aiProgress, aiBusy, refreshAiStatus, refreshFolders, folders, theme, toggleTheme } =
    useApp();
  const [backupMsg, setBackupMsg] = useState<string | null>(null);
  const [tileDefault, setTileDefault] = useState<string>("auto");
  const [fp16Default, setFp16Default] = useState(false);
  const [log, setLog] = useState<string[]>([]);

  useEffect(() => {
    void refreshAiStatus();
    void refreshFolders();
    void api.getSetting("tile_default").then((v) => v && setTileDefault(v));
    void api.getSetting("fp16_default").then((v) => setFp16Default(v === "1"));
  }, [refreshAiStatus, refreshFolders]);

  useEffect(() => {
    if (aiProgress && aiProgress.message) {
      setLog((l) => [...l.slice(-40), `${aiProgress.percent.toFixed(0)}% ${aiProgress.message}`]);
    }
  }, [aiProgress]);

  const runSetup = async () => {
    setLog([]);
    try {
      await api.aiStartSetup();
    } catch (e) {
      setLog((l) => [...l, `Error: ${e}`]);
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="mx-auto max-w-3xl">
        <Section title="Appearance">
          <button
            onClick={toggleTheme}
            className="rounded-lg bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
          >
            Switch to {theme === "dark" ? "light" : "dark"} theme
          </button>
        </Section>

        <Section title="AI engine (local, offline after setup)">
          {aiStatus ? (
            <div className="space-y-2 text-sm">
              <div className="flex items-center gap-2">
                <span className="text-neutral-400">Status:</span>
                <span
                  className={`rounded-full px-2 py-0.5 text-xs font-semibold ${
                    aiStatus.state === "ready"
                      ? aiStatus.mode === "cuda"
                        ? "bg-emerald-500/20 text-emerald-300"
                        : "bg-amber-500/20 text-amber-300"
                      : aiStatus.state === "installing"
                        ? "bg-brand-500/20 text-brand-300"
                        : aiStatus.state === "error"
                          ? "bg-red-500/20 text-red-300"
                          : "bg-neutral-700 text-neutral-300"
                  }`}
                >
                  {aiStatus.state === "ready"
                    ? aiStatus.mode === "cuda"
                      ? "Ready — GPU (CUDA)"
                      : "Ready — CPU fallback"
                    : aiStatus.state}
                </span>
              </div>
              {aiStatus.device_name && (
                <div className="text-xs text-neutral-400">GPU: {aiStatus.device_name}</div>
              )}
              {aiStatus.torch_version && (
                <div className="text-xs text-neutral-500">
                  torch {aiStatus.torch_version}
                  {aiStatus.cuda_version ? ` · CUDA ${aiStatus.cuda_version}` : " · no CUDA"}
                </div>
              )}
              {aiStatus.models.length > 0 && (
                <div className="pt-1 text-xs">
                  <div className="mb-1 text-neutral-400">Models:</div>
                  <ul className="space-y-0.5">
                    {aiStatus.models.map((m) => (
                      <li key={m.name} className="flex items-center gap-2 text-neutral-300">
                        <span className={m.present && m.verified ? "text-emerald-400" : "text-amber-400"}>
                          {m.present ? (m.verified ? "✓ verified" : "⚠ present (unverified)") : "○ not downloaded"}
                        </span>
                        {m.name}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {aiStatus.steps.length > 0 && (
                <ul className="pt-1 text-xs text-neutral-400">
                  {aiStatus.steps.map((s) => (
                    <li key={s.name}>
                      <span className={s.state === "done" ? "text-emerald-400" : s.state === "error" ? "text-red-400" : "text-neutral-500"}>
                        {s.state === "done" ? "✓" : s.state === "error" ? "✗" : s.state === "running" ? "…" : "○"}
                      </span>{" "}
                      {s.name} {s.detail && <span className="text-neutral-500">— {s.detail}</span>}
                    </li>
                  ))}
                </ul>
              )}
              {aiStatus.error && <div className="text-xs text-red-400">{aiStatus.error}</div>}
            </div>
          ) : (
            <div className="text-sm text-neutral-500">Loading…</div>
          )}

          <div className="mt-3 flex flex-wrap gap-2">
            <button
              onClick={runSetup}
              disabled={aiBusy}
              className="rounded-lg bg-brand-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-brand-500 disabled:opacity-40"
            >
              {aiStatus?.state === "ready" ? "Re-run setup" : "Run first-run setup"}
            </button>
            {aiBusy && (
              <button
                onClick={() => void api.aiCancelSetup()}
                className="rounded-lg bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
              >
                Cancel
              </button>
            )}
            <button
              onClick={() => void api.aiSelfTest().then(refreshAiStatus)}
              disabled={aiStatus?.state !== "ready"}
              className="rounded-lg bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700 disabled:opacity-40"
            >
              Re-run AI self-test
            </button>
            <button
              onClick={() => void api.aiOpenRuntimeDir()}
              className="rounded-lg bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
            >
              Open runtime folder
            </button>
          </div>

          {aiBusy && aiProgress && (
            <div className="mt-3">
              <div className="mb-1 flex justify-between text-xs text-neutral-400">
                <span>{aiProgress.message}</span>
                <span>{aiProgress.percent.toFixed(0)}%</span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-neutral-800">
                <div className="h-full bg-brand-500 transition-all" style={{ width: `${aiProgress.percent}%` }} />
              </div>
            </div>
          )}
          {log.length > 0 && (
            <div className="mt-3 max-h-40 overflow-y-auto rounded-lg bg-neutral-950/80 p-2 font-mono text-[11px] text-neutral-400">
              {log.map((l, i) => (
                <div key={i}>{l}</div>
              ))}
            </div>
          )}
          <p className="mt-3 text-[11px] leading-snug text-neutral-500">
            First-run setup downloads a private Python runtime and AI models into{" "}
            <span className="font-mono">{aiStatus?.runtime_dir ?? "%APPDATA%\\PhotoStudio\\ai-runtime"}</span>.
            No system Python, Node.js or CUDA toolkit is required or modified. Downloads are SHA-256 verified.
          </p>
        </Section>

        <Section title="Processing defaults">
          <div className="flex flex-wrap items-center gap-4 text-sm">
            <label className="flex items-center gap-2">
              Default tile size
              <select
                value={tileDefault}
                onChange={(e) => {
                  setTileDefault(e.target.value);
                  void api.setSetting("tile_default", e.target.value);
                }}
                className="rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1 text-sm"
              >
                <option value="auto">Auto</option>
                <option value="128">128</option>
                <option value="256">256</option>
                <option value="512">512</option>
                <option value="0">No tiling</option>
              </select>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={fp16Default}
                onChange={(e) => {
                  setFp16Default(e.target.checked);
                  void api.setSetting("fp16_default", e.target.checked ? "1" : "0");
                }}
                className="accent-brand-500"
              />
              FP16 (experimental)
            </label>
          </div>
        </Section>

        <Section title="Library & data">
          <div className="space-y-1 text-xs text-neutral-400">
            <div>Database: <span className="font-mono">{appInfo?.db_path ?? "…"}</span></div>
            <div>Thumbnail cache: <span className="font-mono">{appInfo?.thumb_dir ?? "…"}</span></div>
            <div>App data: <span className="font-mono">{appInfo?.app_data_dir ?? "…"}</span></div>
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            <button
              onClick={async () => {
                try {
                  const p = await api.backupDatabase();
                  setBackupMsg(`Backup written to ${p}`);
                } catch (e) {
                  setBackupMsg(`Backup failed: ${e}`);
                }
                setTimeout(() => setBackupMsg(null), 8000);
              }}
              className="rounded-lg bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
            >
              Backup database now
            </button>
          </div>
          {backupMsg && <div className="mt-2 text-xs text-emerald-400">{backupMsg}</div>}
          <div className="mt-4">
            <div className="mb-1 text-xs font-semibold uppercase tracking-wider text-neutral-500">Imported folders</div>
            {folders.length === 0 && <div className="text-xs text-neutral-500">None.</div>}
            <ul className="space-y-1">
              {folders.map((f) => (
                <li key={f.id} className="flex items-center gap-2 text-xs text-neutral-300">
                  <span className="min-w-0 flex-1 truncate font-mono">{f.path}</span>
                  <button
                    className="rounded bg-neutral-800 px-2 py-0.5 hover:bg-neutral-700"
                    onClick={async () => {
                      await api.rescanFolder(f.id);
                      void refreshFolders();
                    }}
                  >
                    Rescan
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </Section>

        <Section title="About">
          <div className="text-sm text-neutral-400">
            PhotoStudio {appInfo?.version ?? ""} — local photo library, editor and AI upscaling. All data stays on
            this PC.
          </div>
        </Section>
      </div>
    </div>
  );
}
