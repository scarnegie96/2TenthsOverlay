import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";


// Two pages: the transparent overlay and the settings window.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
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
