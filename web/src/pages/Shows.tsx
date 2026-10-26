import { Link } from "react-router";

import { useShows } from "../api/queries";
import { Card, Load, scoreTone } from "../components/ui";
import { dateTime, euro, number } from "../format";

export function Shows() {
  const shows = useShows();
  return (
    <Card title="Shows">
      <Load query={shows}>
        {(rows) => (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Show</th>
                  <th className="num">Translations</th>
                  <th className="num">Avg score</th>
                  <th className="num hide-sm">Step 2</th>
                  <th className="num hide-sm">Weak</th>
                  <th className="num" title="Every call, retries and replaced translations included">
                    Spent
                  </th>
                  <th className="hide-sm">Last</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((s) => (
                  <tr key={s.show}>
                    <td>
                      <Link to={`/translations?show=${encodeURIComponent(s.show)}`}>{s.show}</Link>
                      {s.show_kind === "movie" && <span className="muted small"> film</span>}
                    </td>
                    <td className="num">{number(s.translations)}</td>
                    <td className={`num tone-text-${scoreTone(s.avg_score) ?? "none"}`}>
                      {number(s.avg_score, 1)}
                      {s.judged > 0 && <span className="muted small"> ({s.judged})</span>}
                    </td>
                    <td className="num hide-sm">{number(s.fallbacks)}</td>
                    <td className={`num hide-sm ${s.weak > 0 ? "tone-text-bad" : ""}`}>{number(s.weak)}</td>
                    <td className="num">{euro(s.eur_spent)}</td>
                    <td className="nowrap muted hide-sm">{dateTime(s.last_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Load>
    </Card>
  );
}
