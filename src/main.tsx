import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { api, onAiEvent } from "./lib/api";
import { useApp } from "./store";

async function bootstrap() {
  try {
    const savedTheme = await api.getSetting("theme");
    const theme = savedTheme === "light" ? "light" : "dark";
    document.documentElement.classList.toggle("light", theme === "light");
    document.documentElement.classList.toggle("dark", theme === "dark");
    useApp.setState({ theme });
  } catch {
    // ignore — default dark
  }
  onAiEvent((evt) => useApp.getState().handleAiEvent(evt)).catch(console.error);

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap();
