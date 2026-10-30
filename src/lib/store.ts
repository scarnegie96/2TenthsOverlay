import { create } from "zustand";
import { api, on } from "./bridge";
import { type AppConfig, defaultConfig, normalizeConfig } from "./config";
import type { FieldPayload, SessionPayload, StatusPayload, TickPayload } from "./types";

interface LiveState {
  status: StatusPayload | null;
  session: SessionPayload | null;
  tick: TickPayload | null;
  field: FieldPayload | null;
  editMode: boolean;
  overlayVisible: boolean;
  config: AppConfig;
  configLoaded: boolean;
}

export const useLive = create<LiveState>(() => ({
  status: null,
  session: null,
  tick: null,
  field: null,
  editMode: false,
  overlayVisible: true,
  config: defaultConfig(),
  configLoaded: false,
}));

/** Subscribes this window to everything the backend sends. Call once per window. */
export async function connect(opts: { telemetry: boolean }) {
  const set = useLive.setState;

  await Promise.all([
    on<StatusPayload>("status", (status) => set({ status })),
    on<SessionPayload>("session", (session) => set({ session })),
    on<FieldPayload>("field", (field) => set({ field })),
    on<boolean>("edit-mode", (editMode) => set({ editMode })),
    on<boolean>("overlay-visible", (overlayVisible) => set({ overlayVisible })),
    on<unknown>("config-changed", (raw) => {
      // Ignore echoes while this window still has edits waiting to be saved.
      if (savePending) return;
      set({ config: normalizeConfig(raw) });
    }),
    opts.telemetry ? on<TickPayload>("tick", (tick) => set({ tick })) : Promise.resolve(),
  ]);

  const [snap, saved] = await Promise.all([api.snapshot(), api.loadConfig()]);
  set({
    status: snap.status ?? useLive.getState().status,
    session: snap.session ?? useLive.getState().session,
    editMode: snap.editMode,
    overlayVisible: snap.overlayVisible,
    config: normalizeConfig(saved),
    configLoaded: true,
  });
  return useLive.getState().config;
}

let saveTimer: number | undefined;
let savePending = false;

/** Updates config locally right away and persists it (debounced). */
export function updateConfig(mutate: (c: AppConfig) => AppConfig) {
  const next = mutate(useLive.getState().config);
  useLive.setState({ config: next });
  window.clearTimeout(saveTimer);
  savePending = true;
  saveTimer = window.setTimeout(() => {
    void api.saveConfig(useLive.getState().config).finally(() => {
      savePending = false;
    });
  }, 150);
}
