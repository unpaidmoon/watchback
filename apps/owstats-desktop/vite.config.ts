import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { rmSync } from "node:fs";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [
    react(),
    {
      name: "remove-non-runtime-source-manifests",
      closeBundle() {
        rmSync(resolve("dist/assets/stadium/manifest.json"), { force: true });
      }
    }
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: false
  }
});
