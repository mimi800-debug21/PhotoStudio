import { useEffect } from "react";
import { useApp, type Route } from "./store";
import Sidebar from "./components/Sidebar";
import TopBar from "./components/TopBar";
import Viewer from "./components/Viewer";
import LibraryView from "./views/LibraryView";
import EditorView from "./views/EditorView";
import EnhanceView from "./views/EnhanceView";
import AlbumsView from "./views/AlbumsView";
import TimelineView from "./views/TimelineView";
import SettingsView from "./views/SettingsView";

const TITLES: Record<Route, string> = {
  library: "Library",
  editor: "Editor",
  enhance: "AI Enhance",
  albums: "Albums",
  timeline: "Timeline",
  settings: "Settings",
};

export default function App() {
  const { route, loadAppInfo, refreshPhotos, refreshFolders, refreshAlbums, refreshAiStatus, viewerOpen } =
    useApp();

  useEffect(() => {
    void loadAppInfo();
    void refreshPhotos();
    void refreshFolders();
    void refreshAlbums();
    void refreshAiStatus();
  }, [loadAppInfo, refreshPhotos, refreshFolders, refreshAlbums, refreshAiStatus]);

  return (
    <div className="flex h-full w-full overflow-hidden">
      <Sidebar />
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <TopBar title={TITLES[route]} />
        <main className="min-h-0 flex-1 overflow-hidden">
          {route === "library" && <LibraryView />}
          {route === "editor" && <EditorView />}
          {route === "enhance" && <EnhanceView />}
          {route === "albums" && <AlbumsView />}
          {route === "timeline" && <TimelineView />}
          {route === "settings" && <SettingsView />}
        </main>
      </div>
      {viewerOpen && <Viewer />}
    </div>
  );
}
