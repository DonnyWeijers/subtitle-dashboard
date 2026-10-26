import { Link, useParams } from "react-router";

import type { TranslationDetail } from "../api/client";
import { useTranslation } from "../api/queries";
import { Badge, Card, Kpi, Load, RepairBadge, StepBadge, scoreTone } from "../components/ui";
import { check, dateTime, episode, euro, model, number, percent } from "../format";

export function Detail() {
  const { id = "" } = useParams();
  const query = useTranslation(id);
  return (
    <>
      <p>
        <Link to="/translations">← Translations</Link>
      </p>
      <Load query={query}>{(t) => <View t={t} />}</Load>
    </>
  );
}

function View({ t }: { t: TranslationDetail }) {
  const finalChecks = Object.entries(t.checks);
  return (
    <>
      <Card>
        <h1 className="title">
          {t.show} <span className="muted">{episode(t)}</span>
        </h1>
        {t.episode_title && <p className="subtitle">{t.episode_title}</p>}
        <p className="tags">
          <StepBadge step={t.step} />
          <RepairBadge value={t.repair} />
          {t.weak && <Badge tone="bad">weak</Badge>}
          <span className="muted small">{dateTime(t.at)}</span>
        </p>
        {t.file && <p className="muted small break">{t.file}</p>}
      </Card>

      <div className="kpis">
        <Kpi label="Score" value={number(t.score)} sub={t.score == null ? "not judged" : `accuracy ${number(t.accuracy)} · fluency ${number(t.fluency)}`} tone={scoreTone(t.score)} />
        <Kpi label="Cost" value={euro(t.eur)} sub={t.judge_eur > 0 ? `${euro(t.judge_eur)} for the judge` : undefined} />
        <Kpi label="Lines" value={number(t.lines)} sub={t.missing > 0 ? `${number(t.missing)} missing` : "none missing"} tone={t.missing > 0 ? "warn" : undefined} />
        <Kpi label="Time" value={t.seconds == null ? "–" : `${number(t.seconds)} s`} sub={t.models_used.map(model).join(", ")} />
      </div>

      <div className="grid-2">
        <Card title="Free checks">
          {finalChecks.length === 0 ? (
            <p className="muted">Translated before the checks existed.</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>Check</th>
                  {t.step1 && <th className="num">Step 1</th>}
                  <th className="num">Final</th>
                </tr>
              </thead>
              <tbody>
                {finalChecks.map(([name, share]) => (
                  <tr key={name}>
                    <td>{check(name)}</td>
                    {t.step1 && (
                      <td className={`num ${t.step1.failed.includes(name) ? "tone-text-bad" : ""}`}>
                        {percent((t.step1.checks[name] ?? 0) * 100, 2)}
                      </td>
                    )}
                    <td className={`num ${t.failed_checks.includes(name) ? "tone-text-bad" : ""}`}>
                      {percent(share * 100, 2)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {t.step1 && t.step1.redo.length > 0 && (
            <>
              <h3>Repair</h3>
              <ol className="steps">
                {t.step1.redo.map((r, i) => (
                  <li key={i}>
                    {r.batches == null ? "Whole file" : `${r.batches} batch${r.batches === 1 ? "" : "es"}`} with{" "}
                    {model(r.model)}
                    {r.effort ? ` (${r.effort})` : ""}:{" "}
                    {r.failed.length === 0 ? (
                      <span className="tone-text-good">passed</span>
                    ) : (
                      <span className="tone-text-bad">still {r.failed.map(check).join(", ").toLowerCase()}</span>
                    )}
                  </li>
                ))}
              </ol>
            </>
          )}
        </Card>

        <Card title="Judge's notes">
          {t.problems.length === 0 ? (
            <p className="muted">{t.score == null ? "Not in the judged sample." : "No problems found."}</p>
          ) : (
            <ul className="problems">
              {t.problems.map((p, i) => (
                <li key={i}>
                  <span className="tags">
                    <Badge tone="warn">{p.kind || "note"}</Badge>
                    {p.line != null && <span className="muted small">line {p.line}</span>}
                  </span>
                  <p>{p.note}</p>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      {t.history.length > 1 && (
        <Card title="Earlier attempts">
          <ul className="list">
            {t.history.map((h, i) => (
              <li key={i} className="list-row">
                <span>{dateTime(h.at)}</span>
                <span className="list-tags">
                  {h.step != null && <StepBadge step={h.step} />}
                  {h.failed.length > 0 && <Badge tone="bad">{h.failed.map(check).join(", ")}</Badge>}
                  <span className="num">{number(h.score)}</span>
                </span>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </>
  );
}
