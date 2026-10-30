// One API for both runtimes:
//  * inside Tauri (Windows app) -> real commands and events
//  * in a plain browser (`npm run dev`, e.g. on a Mac) -> a local mock with a fake race,
//    so the UI can be worked on without Windows or iRacing.

import type { IbtSummary, Snapshot } from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Unlisten = () => void;

export async function on<T>(event: string, handler: (payload: T) => void): Promise<Unlisten> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<T>(event, (e) => handler(e.payload));
  }
  const { mock } = await import("./mock");
  return mock.on(event, handler as (p: unknown) => void);
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const { mock } = await import("./mock");
  return mock.call(cmd, args) as Promise<T>;
}

export const api = {
  loadConfig: () => call<unknown | null>("load_config"),
  saveConfig: (config: unknown) => call<void>("save_config", { config }),
  snapshot: () => call<Snapshot>("get_snapshot"),
  setEditMode: (enabled: boolean) => call<void>("set_edit_mode", { enabled }),
  setOverlayVisible: (visible: boolean) => call<void>("set_overlay_visible", { visible }),
  setDemo: (enabled: boolean) => call<void>("set_demo", { enabled }),
  openSettings: () => call<void>("open_settings"),
  /** Asks for a lap file (.blap, .olap or .ibt). Returns its path, or null if cancelled. */
  pickLapFile: async (): Promise<string | null> => {
    if (!isTauri) return "demo.blap";
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      multiple: false,
      directory: false,
      title: "Choose a lap to compare against",
      filters: [
        { name: "iRacing laps and telemetry", extensions: ["blap", "olap", "ibt"] },
        { name: "Best / optimal lap (.blap, .olap)", extensions: ["blap", "olap"] },
        { name: "Telemetry (.ibt)", extensions: ["ibt"] },
      ],
    });
    return typeof picked === "string" ? picked : null;
  },
  inspectLapFile: (path: string) => call<IbtSummary>("inspect_lap_file", { path }),
  loadReference: (path: string, lap: number) => call<void>("load_reference", { path, lap }),
  clearReference: () => call<void>("clear_reference"),
};
