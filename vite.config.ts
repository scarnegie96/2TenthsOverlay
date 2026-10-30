import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";


// Two pages: the transparent overlay and the settings window.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Cargo's build output lives under the project root; watching it makes
    // Vite crash with EBUSY when Windows locks build-script executables.
    watch: { ignored: ["**/target/**", "**/src-tauri/**"] },
  },
  build: {
    target: "chrome110", // WebView2 is Chromium
    rollupOptions: {
      input: {
        overlay: "overlay.html",
        settings: "settings.html",
      },
    },
  },
});
