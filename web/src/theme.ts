import { useState } from "react";

export type Theme = "dark" | "light";

const KEY = "theme";

/** Dark unless the viewer picked light; the choice is kept in this browser only. */
export function useTheme(): [Theme, () => void] {
  const [theme, setTheme] = useState<Theme>(() =>
    document.documentElement.dataset.theme === "light" ? "light" : "dark",
  );
  const toggle = () => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    if (next === "light") document.documentElement.dataset.theme = "light";
    else delete document.documentElement.dataset.theme;
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // private window or blocked storage: the choice lasts until reload
    }
    setTheme(next);
  };
  return [theme, toggle];
}
