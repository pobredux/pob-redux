/**
 * Number and date formatting bound to the language picked in the app.
 *
 * `toLocaleString()` with no argument follows the operating system, so a UI in
 * one language on an OS set to another prints dates in the wrong language.
 * These read `locale.current` at call time, which is a `$state`, so a switch
 * re-formats whatever is on screen the same way the messages themselves do.
 */
import { locale } from "$lib/state/locale.svelte";

export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return value.toLocaleString(locale.current, options);
}

export function formatDate(value: Date | number | string, options?: Intl.DateTimeFormatOptions): string {
  return new Date(value).toLocaleDateString(locale.current, options);
}

export function formatDateTime(value: Date | number | string, options?: Intl.DateTimeFormatOptions): string {
  return new Date(value).toLocaleString(locale.current, options);
}
