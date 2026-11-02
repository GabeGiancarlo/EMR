import { useEffect, useState } from "react";
import {
  applyTheme,
  persistTheme,
  readStoredTheme,
  themeAfterOsChange,
  watchSystemTheme,
  type ThemeMode,
} from "./theme";

const OPTIONS: { id: ThemeMode; label: string }[] = [
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
  { id: "system", label: "System" },
];

export default function ThemeControl() {
  const [mode, setMode] = useState<ThemeMode>(readStoredTheme);

  useEffect(() => {
    applyTheme(mode);
    persistTheme(mode);
    if (mode !== "system") return;
    return watchSystemTheme((osDark) => {
      const next = themeAfterOsChange("system", osDark);
      applyTheme(next.mode, osDark);
    });
  }, [mode]);

  return (
    <fieldset className="theme-control">
      <legend>Color theme</legend>
      <div className="theme-control-options">
        {OPTIONS.map((option) => (
          <label key={option.id} className={mode === option.id ? "active" : ""}>
            <input
              type="radio"
              name="nexus-theme-mode"
              value={option.id}
              checked={mode === option.id}
              onChange={() => setMode(option.id)}
            />
            {option.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}
