import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore } from "./store";
import Popup from "./popup/Popup";
import SettingsView from "./settings/Settings";
import StatsView from "./stats/Stats";

export default function App() {
  const label = getCurrentWindow().label;
  const settings = useStore((s) => s.settings);
  const init = useStore((s) => s.init);

  useEffect(() => init(), [init]);

  useEffect(() => {
    const theme = settings?.theme ?? "system";
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () =>
      document.documentElement.classList.toggle(
        "dark",
        theme === "dark" || (theme === "system" && media.matches),
      );
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings?.theme]);

  if (label === "settings") return <SettingsView />;
  if (label === "stats") return <StatsView />;
  return <Popup />;
}
