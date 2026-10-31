// One API for both runtimes:
//  * inside Tauri (Windows app) -> real commands and events
//  * in a plain browser (`npm run dev`, e.g. on a Mac) -> a local mock with a fake race,
//    so the UI can be worked on without Windows or iRacing.

import type { ActionId, ControlBinding } from "./controls";
import type { Controller, IbtSummary, PitCommand, Profile, ProfileSummary, Snapshot } from "./types";

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
  pitCommand: (action: PitCommand) => call<void>("pit_command", { action }),
  /** Pit hotkeys and buttons only work while this is on (widget on, you in the car). */
  setPitActive: (active: boolean) => call<void>("set_pit_active", { active }),
  /** The key and button for each action. The overlay sends these whenever they change. */
  setControls: (controls: Record<ActionId, ControlBinding>) => call<void>("set_controls", { controls }),
  /** Pauses every hotkey and button while Settings waits for a new one. */
  suspendControls: (suspended: boolean) => call<void>("suspend_controls", { suspended }),
  listControllers: () => call<Controller[]>("list_controllers"),
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

  listProfiles: () => call<ProfileSummary[]>("list_profiles"),
  getProfile: (id: string) => call<Profile>("get_profile", { id }),
  /** Creates a layout (id null) or updates one (name null keeps its name). Returns its id. */
  saveProfile: (id: string | null, name: string | null, widgets: unknown) => call<string>("save_profile", { id, name, widgets }),
  deleteProfile: (id: string) => call<void>("delete_profile", { id }),
  /** Asks where to save a copy of a layout. Returns false if cancelled. */
  exportProfile: async (id: string, name: string): Promise<boolean> => {
    const fileName = `${name.replace(/[\\/:*?"<>|]+/g, "-")}.tenths-layout.json`;
    if (!isTauri) {
      const profile = await call<Profile>("get_profile", { id });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(new Blob([JSON.stringify(profile, null, 2)], { type: "application/json" }));
      a.download = fileName;
      a.click();
      URL.revokeObjectURL(a.href);
      return true;
    }
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({ title: "Export layout", defaultPath: fileName, filters: [{ name: "Tenths layout", extensions: ["json"] }] });
    if (!path) return false;
    await call<void>("export_profile", { id, path });
    return true;
  },
  /** Asks for a layout file and adds it. Returns its new id, or null if cancelled. */
  importProfile: async (): Promise<string | null> => {
    if (!isTauri) {
      const text = await new Promise<string | null>((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = ".json,application/json";
        input.onchange = () => (input.files?.[0] ? input.files[0].text().then(resolve) : resolve(null));
        input.click();
      });
      return text == null ? null : call<string>("import_profile_text", { text });
    }
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({ multiple: false, directory: false, title: "Import a layout", filters: [{ name: "Tenths layout", extensions: ["json"] }] });
    return typeof path === "string" ? call<string>("import_profile", { path }) : null;
  },
};
