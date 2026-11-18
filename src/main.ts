// First import: it overwrites Paraglide's getLocale() before any component module loads.
import "$lib/state/locale.svelte";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "./app.css";
import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.svelte";
import AssistantWindow from "./AssistantWindow.svelte";
import ItemWindow from "./ItemWindow.svelte";
import { IS_ASSISTANT_WINDOW, IS_ITEM_WINDOW } from "$lib/window";

const logError = (message: string) => void invoke("log_frontend", { level: "error", message }).catch(() => {});
window.addEventListener("error", (e) => logError(`${e.message} (${e.filename}:${e.lineno}:${e.colno})`));
window.addEventListener("unhandledrejection", (e) => {
  const r = e.reason;
  logError(`unhandled rejection: ${r instanceof Error ? (r.stack ?? r.message) : String(r)}`);
});

const app = mount(IS_ASSISTANT_WINDOW ? AssistantWindow : IS_ITEM_WINDOW ? ItemWindow : App, { target: document.getElementById("app")! });

export default app;
