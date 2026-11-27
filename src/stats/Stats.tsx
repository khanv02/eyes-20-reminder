import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useStore } from "../store";

type Day = {
  date: string;
  reminders: number;
  done: number;
  activeSecs: number;
  pomodoroCycles: number;
};

type Payload = { today: Day; days: Day[]; streak: number };

export default function StatsView() {
  const [data, setData] = useState<Payload | null>(null);
  const lang = useStore((s) => s.lang);
  const t = useStore((s) => s.t);

  useEffect(() => {
    let alive = true;
    const load = () =>
      invoke<Payload>("get_stats").then((payload) => {
        if (alive) setData(payload);
      });
    load();
    const unlisten = listen("stats:changed", load);
    return () => {
      alive = false;
      unlisten.then((off) => off());
    };
  }, []);

  const duration = (secs: number) => {
    const minutes = Math.round(secs / 60);
    if (minutes < 60) return t("stats.minutes", { n: minutes });
    return t("stats.hours", { h: Math.floor(minutes / 60), m: minutes % 60 });
  };

  const days = data?.days ?? [];
  const max = Math.max(1, ...days.map((d) => d.done));

  return (
    <div className="window-surface min-h-screen px-5 py-4 text-sm">
      <h1 className="text-base font-semibold">BreakBuddy · {t("stats.title")}</h1>

      <div className="mt-3 grid grid-cols-2 gap-2">
        <Card label={t("stats.reminders")} value={String(data?.today.reminders ?? 0)} />
        <Card label={t("stats.done")} value={String(data?.today.done ?? 0)} />
        <Card label={t("stats.active")} value={duration(data?.today.activeSecs ?? 0)} />
        <Card label={t("stats.cycles")} value={String(data?.today.pomodoroCycles ?? 0)} />
      </div>

      <section className="mt-4 rounded-xl border border-slate-200 bg-white/70 p-3 dark:border-slate-700 dark:bg-slate-900/60">
        <h2 className="text-[12px] font-semibold uppercase tracking-wide text-emerald-700 dark:text-emerald-300">
          {t("stats.week")}
        </h2>
        {days.every((d) => d.reminders === 0) ? (
          <p className="mt-2 text-[12px] text-slate-500 dark:text-slate-400">{t("stats.empty")}</p>
        ) : (
          <div className="mt-3 flex h-28 items-end gap-1.5">
            {days.map((day) => {
              const label = new Date(`${day.date}T00:00:00`).toLocaleDateString(
                lang === "vi" ? "vi-VN" : "en-US",
                { weekday: "short" },
              );
              return (
                <div key={day.date} className="flex flex-1 flex-col items-center gap-1">
                  <div className="flex w-full flex-1 items-end">
                    <div
                      title={`${day.date}: ${day.done}`}
                      style={{ height: `${Math.max(4, (day.done / max) * 100)}%` }}
                      className={`w-full rounded-t-md ${
                        day.done > 0 ? "bg-emerald-500" : "bg-emerald-500/20"
                      }`}
                    />
                  </div>
                  <span className="text-[10px] text-slate-500 dark:text-slate-400">{label}</span>
                </div>
              );
            })}
          </div>
        )}
      </section>

      <div className="mt-3 flex items-center justify-between rounded-xl border border-emerald-500/30 bg-emerald-50 px-4 py-3 dark:bg-emerald-500/10">
        <div>
          <p className="text-[12px] text-emerald-800 dark:text-emerald-200">{t("stats.streak")}</p>
          <p className="text-xs text-emerald-700/80 dark:text-emerald-300/80">
            {t("stats.streakDays", { n: data?.streak ?? 0 })}
          </p>
        </div>
        <span className="text-2xl font-semibold text-emerald-700 tabular-nums dark:text-emerald-300">
          🔥 {data?.streak ?? 0}
        </span>
      </div>
    </div>
  );
}

function Card({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-slate-200 bg-white/70 px-3 py-2 dark:border-slate-700 dark:bg-slate-900/60">
      <p className="text-[11px] text-slate-500 dark:text-slate-400">{label}</p>
      <p className="mt-0.5 text-lg font-semibold tabular-nums">{value}</p>
    </div>
  );
}
