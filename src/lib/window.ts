import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ViewId } from "$lib/state/build.svelte";

export const WINDOW_LABEL = getCurrentWindow().label;

export const IS_MAIN_WINDOW = WINDOW_LABEL === "main";

/** This webview is the assistant popped out of the main window (`assistant.rs`). */
export const IS_ASSISTANT_WINDOW = WINDOW_LABEL === "assistant";

/** This webview is an item popped out of the Items view (`item_window.rs`). */
export const IS_ITEM_WINDOW = WINDOW_LABEL === "item";

/** The tab this webview holds when it is a tab torn off the main window (`tab_window.rs`). */
export const TAB_VIEW = WINDOW_LABEL.startsWith("tab-") ? (WINDOW_LABEL.slice(4) as ViewId) : null;
