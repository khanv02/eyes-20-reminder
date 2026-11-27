import { invoke } from "@tauri-apps/api/core";
import type { ReactNode } from "react";
import { useStore } from "../store";

const field =
  "rounded-lg border border-slate-200 bg-white px-2 py-1 text-[13px] text-slate-800 outline-none focus:border-emerald-500 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100";

export default function SettingsView() {
  const settings = useStore((s) => s.settings);
  const update = useStore((s) => s.update);
  const t = useStore((s) => s.t);

  if (!settings) return null;
  const set = (patch: Record<string, unknown>) => update(patch);

  return (
    <div className="window-surface min-h-screen px-5 py-4 text-sm">
      <header className="mb-4 flex items-center justify-between gap-3">
        <h1 className="text-base font-semibold">BreakBuddy · {t("settings.title")}</h1>
        <button
          onClick={() => invoke("open_stats")}
          className="rounded-lg bg-emerald-600 px-3 py-1.5 text-[12px] font-medium text-white hover:bg-emerald-700"
        >
          {t("settings.openStats")}
        </button>
      </header>

      <div className="space-y-4">
        <Section title={t("settings.general")}>
          <Row label={t("settings.mode")}>
            <select
              value={settings.mode}
              onChange={(e) => set({ mode: e.target.value })}
              className={field}
            >
              <option value="reminder">{t("settings.modeReminder")}</option>
              <option value="pomodoro">{t("settings.modePomodoro")}</option>
            </select>
          </Row>
          <Toggle
            label={t("settings.autostart")}
            checked={settings.autostart}
            onChange={(autostart) => set({ autostart })}
          />
          <Toggle
            label={t("settings.workHours")}
            checked={settings.workHours !== null}
            onChange={(on) => set({ workHours: on ? { start: "09:00", end: "18:00" } : null })}
          />
          {settings.workHours && (
            <Row label={`${t("settings.start")} – ${t("settings.end")}`}>
              <div className="flex items-center gap-1">
                <input
                  type="time"
                  value={settings.workHours.start}
                  onChange={(e) =>
                    set({ workHours: { ...settings.workHours!, start: e.target.value } })
                  }
                  className={field}
                />
                <input
                  type="time"
                  value={settings.workHours.end}
                  onChange={(e) =>
                    set({ workHours: { ...settings.workHours!, end: e.target.value } })
                  }
                  className={field}
                />
              </div>
            </Row>
          )}
        </Section>

        <Section title={t("settings.reminders")}>
          <NumberRow
            label={t("settings.workMinutes")}
            value={settings.workMinutes}
            min={1}
            max={180}
            onChange={(workMinutes) => set({ workMinutes })}
          />
          <NumberRow
            label={t("settings.idleThreshold")}
            value={settings.idleThresholdSec}
            min={10}
            max={1800}
            onChange={(idleThresholdSec) => set({ idleThresholdSec })}
          />
          <NumberRow
            label={t("settings.naturalBreak")}
            value={settings.naturalBreakSec}
            min={30}
            max={3600}
            onChange={(naturalBreakSec) => set({ naturalBreakSec })}
          />
          <Row label={t("settings.corner")}>
            <select
              value={settings.popupCorner}
              onChange={(e) => set({ popupCorner: e.target.value })}
              className={field}
            >
              <option value="bottom-right">{t("settings.bottomRight")}</option>
              <option value="bottom-left">{t("settings.bottomLeft")}</option>
              <option value="top-right">{t("settings.topRight")}</option>
              <option value="top-left">{t("settings.topLeft")}</option>
            </select>
          </Row>
          <Toggle
            label={t("settings.sound")}
            checked={settings.sound}
            onChange={(sound) => set({ sound })}
          />
          <Toggle
            label={t("settings.snooze")}
            checked={settings.snoozeEnabled}
            onChange={(snoozeEnabled) => set({ snoozeEnabled })}
          />
          <NumberRow
            label={t("settings.snoozeMinutes")}
            value={settings.snoozeMinutes}
            min={1}
            max={60}
            onChange={(snoozeMinutes) => set({ snoozeMinutes })}
          />
          <Toggle
            label={t("settings.dnd")}
            checked={settings.doNotDisturbFullscreen}
            onChange={(doNotDisturbFullscreen) => set({ doNotDisturbFullscreen })}
          />
        </Section>

        <Section title={t("settings.pomodoro")}>
          <NumberRow
            label={t("settings.focus")}
            value={settings.pomodoro.focus}
            min={1}
            max={120}
            onChange={(focus) => set({ pomodoro: { ...settings.pomodoro, focus } })}
          />
          <NumberRow
            label={t("settings.short")}
            value={settings.pomodoro.short}
            min={1}
            max={60}
            onChange={(short) => set({ pomodoro: { ...settings.pomodoro, short } })}
          />
          <NumberRow
            label={t("settings.long")}
            value={settings.pomodoro.long}
            min={1}
            max={120}
            onChange={(long) => set({ pomodoro: { ...settings.pomodoro, long } })}
          />
          <NumberRow
            label={t("settings.cycles")}
            value={settings.pomodoro.cycles}
            min={1}
            max={12}
            onChange={(cycles) => set({ pomodoro: { ...settings.pomodoro, cycles } })}
          />
        </Section>

        <Section title={t("settings.appearance")}>
          <Row label={t("settings.theme")}>
            <select
              value={settings.theme}
              onChange={(e) => set({ theme: e.target.value })}
              className={field}
            >
              <option value="system">{t("settings.themeSystem")}</option>
              <option value="light">{t("settings.themeLight")}</option>
              <option value="dark">{t("settings.themeDark")}</option>
            </select>
          </Row>
          <Row label={t("settings.language")}>
            <select
              value={settings.language}
              onChange={(e) => set({ language: e.target.value })}
              className={field}
            >
              <option value="en">English</option>
              <option value="vi">Tiếng Việt</option>
            </select>
          </Row>
        </Section>
      </div>

      <p className="mt-4 text-[11px] text-slate-500 dark:text-slate-400">
        {t("settings.instant")}
      </p>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-xl border border-slate-200 bg-white/70 p-3 dark:border-slate-700 dark:bg-slate-900/60">
      <h2 className="mb-1 text-[12px] font-semibold uppercase tracking-wide text-emerald-700 dark:text-emerald-300">
        {title}
      </h2>
      <div className="divide-y divide-slate-100 dark:divide-slate-800">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 py-1.5">
      <span className="text-slate-600 dark:text-slate-300">{label}</span>
      {children}
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-center justify-between gap-3 py-1.5">
      <span className="text-slate-600 dark:text-slate-300">{label}</span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="size-4 accent-emerald-600"
      />
    </label>
  );
}

function NumberRow({
  label,
  value,
  min,
  max,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <Row label={label}>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        onChange={(e) => {
          const next = Number(e.target.value);
          if (Number.isFinite(next) && next >= min && next <= max) onChange(next);
        }}
        className={`${field} w-20 text-right tabular-nums`}
      />
    </Row>
  );
}

