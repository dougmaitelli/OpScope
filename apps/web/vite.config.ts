import { defineConfig } from "vite";

export default defineConfig({
  server: {
    strictPort: true,
    watch: {
      ignored: ["**/apps/desktop/**"],
    },
    proxy: {
      "/api": "http://127.0.0.1:4317",
    },
  },
});
