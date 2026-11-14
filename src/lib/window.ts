import { getCurrentWindow } from "@tauri-apps/api/window";

/** This webview is the assistant popped out of the main window (`assistant.rs`). */
export const IS_ASSISTANT_WINDOW = getCurrentWindow().label === "assistant";
