import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: false,
    fs: {
      allow: ["D:/Codex/OW_data/owstats-local/apps/owstats-desktop", "D:/Codex/OW_data/owstats-local/assets"]
    }
  }
});
