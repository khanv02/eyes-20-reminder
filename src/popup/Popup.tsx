import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useEffect, useRef } from "react";
import { mmss, useStore } from "../store";

export default function Popup() {
  const settings = useStore((s) => s.settings);
  const snapshot = useStore((s) => s.snapshot);
  const reminder = useStore((s) => s.reminder);
  const nudges = useStore((s) => s.nudges);
  const t = useStore((s) => s.t);
  const audio = useRef<HTMLAudioElement>(null);

  const kind = reminder?.kind ?? "work";
  const pomodoro = settings?.mode === "pomodoro";
  const done = () => invoke(kind === "break" ? "start_break" : "complete_break");
  const snooze = () => invoke("snooze", { minutes: settings?.snoozeMinutes ?? 5 });

  useEffect(() => {
    if (!settings?.sound) return;
    // A missing output device must never break the reminder.
    audio.current?.play().catch(() => {});
  }, [reminder, nudges, settings?.sound]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Enter") done();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const title =
    kind === "work"
      ? t("popup.workTitle", { minutes: settings?.workMinutes ?? 20 })
      : kind === "break"
        ? t("popup.breakTitle")
        : t("popup.resumeTitle");
  const tip = reminder?.tip ? t(`tips.${reminder.tip}`) : t("popup.workBody");
  const action = kind === "break" ? t("popup.startBreak") : t("popup.done");
  const showSnooze = kind !== "break" && settings?.snoozeEnabled;

  return (
    <div key={nudges} className={nudges > 0 ? "animate-[shake_0.5s_ease-in-out]" : undefined}>
      <audio ref={audio} src="/chime.wav" preload="auto" />
      <motion.div
        initial={{ opacity: 0, y: 18, scale: 0.96 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.25, ease: "easeOut" }}
        className="p-2"
      >
        <div className="rounded-2xl bg-white/95 p-4 shadow-xl ring-1 ring-emerald-500/20 backdrop-blur dark:bg-slate-900/95 dark:ring-emerald-400/20">
          <div className="flex items-start gap-3">
            <motion.div
              aria-hidden
              animate={{ y: [0, -3, 0] }}
              transition={{ duration: 2.4, repeat: Infinity, ease: "easeInOut" }}
              className="grid size-9 shrink-0 place-items-center rounded-xl bg-emerald-100 text-xl dark:bg-emerald-500/15"
            >
              {kind === "break" ? "🌿" : "🙆"}
            </motion.div>
            <div className="min-w-0 flex-1">
              <p className="text-[15px] font-semibold leading-snug">{title}</p>
              <p className="mt-1 text-[13px] leading-snug text-slate-600 dark:text-slate-300">
                {kind === "work" ? tip : t(kind === "break" ? "popup.breakBody" : "popup.resumeBody")}
              </p>
              {pomodoro && snapshot && (
                <p className="mt-1 text-[11px] tracking-wide text-emerald-700 dark:text-emerald-300">
                  {"🍅".repeat(snapshot.cycle)}
                  {"⚪".repeat(Math.max(0, snapshot.cycles - snapshot.cycle))}{" "}
                  {t("popup.cycle", { n: snapshot.cycle, total: snapshot.cycles })}
                </p>
              )}
            </div>
            {pomodoro && (
              <Ring progress={snapshot?.progress ?? 0} label={mmss(snapshot?.remainingSec ?? 0)} />
            )}
          </div>

          <div className="mt-3 flex flex-col gap-1">
            <button
              onClick={done}
              className="w-full rounded-xl bg-emerald-600 px-4 py-2 text-[14px] font-semibold text-white transition hover:bg-emerald-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-emerald-400"
            >
              {action}
            </button>
            {showSnooze && (
              <button
                onClick={snooze}
                className="rounded-lg py-1 text-[12px] text-slate-500 transition hover:text-emerald-700 dark:text-slate-400 dark:hover:text-emerald-300"
              >
                {t("popup.snooze", { minutes: settings.snoozeMinutes })}
              </button>
            )}
          </div>
        </div>
      </motion.div>
    </div>
  );
}

function Ring({ progress, label }: { progress: number; label: string }) {
  const radius = 20;
  const circumference = 2 * Math.PI * radius;
  return (
    <div className="relative grid size-14 shrink-0 place-items-center">
      <svg viewBox="0 0 48 48" className="absolute inset-0 -rotate-90">
        <circle cx="24" cy="24" r={radius} fill="none" strokeWidth="4" className="stroke-emerald-100 dark:stroke-emerald-500/20" />
        <circle
          cx="24"
          cy="24"
          r={radius}
          fill="none"
          strokeWidth="4"
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - progress)}
          className="stroke-emerald-500 transition-[stroke-dashoffset] duration-500"
        />
      </svg>
      <span className="text-[11px] font-semibold tabular-nums">{label}</span>
    </div>
  );
}
