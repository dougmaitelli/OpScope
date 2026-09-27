import js from "@eslint/js";
import { defineConfig, globalIgnores } from "eslint/config";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

export default defineConfig([
  globalIgnores(["dist", "src/generated/**"]),
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: "latest",
      globals: globals.browser,
    },
    rules: {
      // Resource loaders own their loading state and are intentionally started by effects.
      "react-hooks/set-state-in-effect": "off",
      // These helpers share state with their colocated provider or component.
      "react-refresh/only-export-components": [
        "error",
        {
          allowConstantExport: true,
          allowExportNames: [
            "groupWorkflows",
            "isDesktopRuntime",
            "setHttpCsrfToken",
            "useApplicationClient",
            "useWebAuthentication",
          ],
        },
      ],
    },
  },
]);
