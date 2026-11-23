import { locale } from "$lib/state/locale.svelte";

// toLocaleString() with no locale follows the OS language, not the one picked in the app.
export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return value.toLocaleString(locale.current, options);
}

export function formatDate(value: Date | number | string, options?: Intl.DateTimeFormatOptions): string {
  return new Date(value).toLocaleDateString(locale.current, options);
}

export function formatDateTime(value: Date | number | string, options?: Intl.DateTimeFormatOptions): string {
  return new Date(value).toLocaleString(locale.current, options);
}
