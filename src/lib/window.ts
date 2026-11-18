import { getCurrentWindow } from "@tauri-apps/api/window";

/** This webview is the assistant popped out of the main window (`assistant.rs`). */
export const IS_ASSISTANT_WINDOW = getCurrentWindow().label === "assistant";

/** This webview is an item popped out of the Items view (`item_window.rs`). */
export const IS_ITEM_WINDOW = getCurrentWindow().label === "item";
