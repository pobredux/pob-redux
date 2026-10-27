import { defineConfig } from "@inlang/paraglide-js";

// Read by both the Vite plugin and `paraglide-js compile`. Paths resolve against the repository root, not this file.
export default defineConfig({
  outdir: "./src/lib/paraglide",
  // Must match the i18n script's --output-structure, or dev recompiles slowly after each check or build.
  outputStructure: "locale-modules",
  // The locale store overwrites getLocale(), so this chain only seeds the first run.
  strategy: ["localStorage", "preferredLanguage", "baseLocale"],
  localStorageKey: "pob-redux:locale",
  emitTsDeclarations: true,
  isServer: "false",
});
