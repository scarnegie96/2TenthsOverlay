import { create } from "zustand";
import { api, isTauri, on } from "./bridge";
import { type AppConfig, defaultConfig, normalizeConfig } from "./config";
import { applyTheme } from "./theme";
import type { FieldPayload, ProfileSummary, ReferencePayload, SessionPayload, StatusPayload, TickPayload } from "./types";

interface LiveState {
  status: StatusPayload | null;
  session: SessionPayload | null;
  reference: ReferencePayload | null;
  tick: TickPayload | null;
  field: FieldPayload | null;
  editMode: boolean;
  overlayVisible: boolean;
  config: AppConfig;
  configLoaded: boolean;
  /** Saved layouts, by name. */
  profiles: ProfileSummary[];
}

export const useLive = create<LiveState>(() => ({
  status: null,
  session: null,
  reference: null,
  tick: null,
  field: null,
  editMode: false,
  overlayVisible: true,
  config: defaultConfig(),
  configLoaded: false,
  profiles: [],
}));

/** Subscribes this window to everything the backend sends. Call once per window. */
export async function connect(opts: { telemetry: boolean }) {
  const set = useLive.setState;
  // Browser preview: ?theme=light for screenshots.
  const forcedTheme = !isTauri ? new URLSearchParams(location.search).get("theme") : null;
  const theme = (g: AppConfig["general"]) =>
    applyTheme(forcedTheme === "light" || forcedTheme === "dark" ? { ...g, theme: forcedTheme } : g);
  theme(useLive.getState().config.general);
  useLive.subscribe((s, prev) => {
    if (s.config.general !== prev.config.general) theme(s.config.general);
  });

  await Promise.all([
    on<StatusPayload>("status", (status) => set({ status })),
    on<SessionPayload>("session", (session) => set({ session })),
    on<ReferencePayload>("reference", (reference) => set({ reference })),
    on<FieldPayload>("field", (field) => set({ field })),
    on<boolean>("edit-mode", (editMode) => set({ editMode })),
    on<boolean>("overlay-visible", (overlayVisible) => set({ overlayVisible })),
    on<ProfileSummary[]>("profiles-changed", (profiles) => set({ profiles })),
    on<unknown>("config-changed", (raw) => {
      // Ignore echoes while this window still has edits waiting to be saved.
      if (savePending) return;
      set({ config: normalizeConfig(raw) });
    }),
    opts.telemetry ? on<TickPayload>("tick", (tick) => set({ tick })) : Promise.resolve(),
  ]);

  const [snap, saved, profiles] = await Promise.all([api.snapshot(), api.loadConfig(), api.listProfiles().catch(() => [])]);
  set({
    status: snap.status ?? useLive.getState().status,
    session: snap.session ?? useLive.getState().session,
    reference: snap.reference ?? useLive.getState().reference,
    editMode: snap.editMode,
    overlayVisible: snap.overlayVisible,
    config: normalizeConfig(saved),
    configLoaded: true,
    profiles,
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
  saveTimer = window.setTimeout(flushSave, 150);
}

function flushSave() {
  window.clearTimeout(saveTimer);
  const config = useLive.getState().config;
  void saveIntoActiveProfile(config);
  return api.saveConfig(config).finally(() => {
    savePending = false;
  });
}

// --- Saved layouts --------------------------------------------------------------

/** Widgets as last written to each layout, so unchanged ones aren't rewritten. */
const savedWidgets = new Map<string, string>();

/** Changes to the widgets go into the layout they came from. */
async function saveIntoActiveProfile(config: AppConfig) {
  const id = config.general.activeProfile;
  if (!id) return;
  const json = JSON.stringify(config.widgets);
  if (savedWidgets.get(id) === json) return;
  savedWidgets.set(id, json);
  try {
    await api.saveProfile(id, null, config.widgets);
  } catch {
    // Deleted elsewhere: carry on with an unsaved layout.
    savedWidgets.delete(id);
    if (useLive.getState().config.general.activeProfile === id) setActive(null);
  }
}

const setActive = (activeProfile: string | null) =>
  updateConfig((c) => ({ ...c, general: { ...c.general, activeProfile } }));

/** Puts a saved layout on screen. */
export async function switchProfile(id: string) {
  if (savePending) await flushSave();
  const profile = await api.getProfile(id);
  const widgets = normalizeConfig({ widgets: profile.widgets }).widgets;
  savedWidgets.set(id, JSON.stringify(widgets));
  updateConfig((c) => ({ ...c, widgets, general: { ...c.general, activeProfile: id } }));
}

/** Saves what's on screen as a new layout and makes it the active one. */
export async function saveNewProfile(name: string) {
  if (savePending) await flushSave();
  const { widgets } = useLive.getState().config;
  const id = await api.saveProfile(null, name, widgets);
  savedWidgets.set(id, JSON.stringify(widgets));
  setActive(id);
  return id;
}

export async function deleteProfile(id: string) {
  await api.deleteProfile(id);
  savedWidgets.delete(id);
  // The widgets stay as they are, just no longer saved anywhere.
  if (useLive.getState().config.general.activeProfile === id) setActive(null);
}

/** For the shortcuts: the next layout in the list, or the one at a position. */
export async function cycleProfile(target: { step?: number | null; index?: number | null }) {
  const { profiles, config } = useLive.getState();
  if (!profiles.length) return;
  let i: number;
  if (target.index != null) {
    if (target.index >= profiles.length) return;
    i = target.index;
  } else {
    const at = profiles.findIndex((p) => p.id === config.general.activeProfile);
    i = (((at < 0 ? -1 : at) + (target.step ?? 1)) % profiles.length + profiles.length) % profiles.length;
  }
  if (profiles[i].id !== config.general.activeProfile) await switchProfile(profiles[i].id);
}
