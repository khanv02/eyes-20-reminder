import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { translate, type Lang } from "./i18n";

export type Settings = {
  mode: "reminder" | "pomodoro";
  workMinutes: number;
  idleThresholdSec: number;
  naturalBreakSec: number;
  popupCorner: "top-left" | "top-right" | "bottom-left" | "bottom-right";
  sound: boolean;
  snoozeEnabled: boolean;
  snoozeMinutes: number;
  pomodoro: { focus: number; short: number; long: number; cycles: number };
  autostart: boolean;
  theme: "light" | "dark" | "system";
  language: Lang;
  workHours: { start: string; end: string } | null;
  doNotDisturbFullscreen: boolean;
};

export type Snapshot = {
  state: string;
  activeSec: number;
  remainingSec: number;
  progress: number;
  cycle: number;
  cycles: number;
  popup: boolean;
};

export type Reminder = { kind: "work" | "break" | "resume"; tip: string };

type Store = {
  settings: Settings | null;
  snapshot: Snapshot | null;
  reminder: Reminder | null;
  nudges: number;
  lang: Lang;
  t: (key: string, vars?: Record<string, string | number>) => string;
  /** Subscribes to Rust events; returns the unsubscribe function. */
  init: () => () => void;
  update: (patch: Record<string, unknown>) => Promise<void>;
};

export const mmss = (secs: number) => {
  const safe = Math.max(0, Math.round(secs));
  return `${String(Math.floor(safe / 60)).padStart(2, "0")}:${String(safe % 60).padStart(2, "0")}`;
};

export const useStore = create<Store>((set, get) => ({
  settings: null,
  snapshot: null,
  reminder: null,
  nudges: 0,
  lang: "en",
  t: (key, vars) => translate(get().lang, key, vars),
  update: async (patch) => {
    const settings = await invoke<Settings>("update_settings", { patch });
    set({ settings, lang: settings.language });
  },
  init: () => {
    let disposed = false;

    invoke<Settings>("get_settings").then((settings) => {
      if (!disposed) set({ settings, lang: settings.language });
    });
    invoke<Snapshot>("get_snapshot").then((snapshot) => {
      if (!disposed) set({ snapshot });
    });
    invoke<Reminder | null>("get_reminder").then((reminder) => {
      if (!disposed) set({ reminder });
    });

    const subscriptions = Promise.all([
      listen<Snapshot>("timer:tick", (event) => set({ snapshot: event.payload })),
      listen<Reminder>("timer:remind", (event) =>
        set({ reminder: event.payload, nudges: 0 }),
      ),
      listen("timer:nudge", () => set((state) => ({ nudges: state.nudges + 1 }))),
      listen<Settings>("settings:changed", (event) =>
        set({ settings: event.payload, lang: event.payload.language }),
      ),
    ]);

    return () => {
      disposed = true;
      subscriptions.then((unlisten) => unlisten.forEach((fn) => fn()));
    };
  },
}));
