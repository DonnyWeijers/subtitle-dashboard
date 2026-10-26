import { NavLink, Route, Routes } from "react-router";

import { Checks } from "./pages/Checks";
import { Detail } from "./pages/Detail";
import { Overview } from "./pages/Overview";
import { Shows } from "./pages/Shows";
import { Translations } from "./pages/Translations";

const NAV = [
  ["/", "Overview"],
  ["/translations", "Translations"],
  ["/checks", "Checks"],
  ["/shows", "Shows"],
] as const;

export function App() {
  return (
    <>
      <header className="top">
        <div className="top-inner">
          <span className="brand">Subtitles</span>
          <nav>
            {NAV.map(([to, label]) => (
              <NavLink key={to} to={to} end={to === "/"}>
                {label}
              </NavLink>
            ))}
          </nav>
        </div>
      </header>
      <main>
        <Routes>
          <Route path="/" element={<Overview />} />
          <Route path="/translations" element={<Translations />} />
          <Route path="/translations/:id" element={<Detail />} />
          <Route path="/checks" element={<Checks />} />
          <Route path="/shows" element={<Shows />} />
          <Route path="*" element={<p className="muted">Page not found.</p>} />
        </Routes>
      </main>
    </>
  );
}
