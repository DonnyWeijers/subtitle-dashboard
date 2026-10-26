import { Link } from "react-router";
import { Bar, BarChart, CartesianGrid, Legend, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

import type { CostRow, Summary } from "../api/client";
import { useCosts, useSummary, useTranslations } from "../api/queries";
import { Card, Kpi, Load, RepairBadge, StepBadge, modelColor, scoreTone } from "../components/ui";
import { dateTime, dayRange, daysAgo, episode, euro, isoDay, model, number, percent, shortDate } from "../format";

const DAYS = 30;

export function Overview() {
  const summary = useSummary();
  const from = daysAgo(DAYS - 1);
  const costs = useCosts({ group: "day", from });
  const recent = useTranslations({ per_page: 8 });

  return (
    <>
      <Load query={summary}>{(s) => <Kpis s={s} />}</Load>

      <div className="grid-2">
        <Card title={`Costs per day, last ${DAYS} days`}>
          <Load query={costs}>{(rows) => <CostChart rows={rows} from={from} />}</Load>
        </Card>
        <Card title="Quality scores">
          <Load query={summary}>{(s) => <ScoreChart s={s} />}</Load>
        </Card>
      </div>

      <Card title="Latest translations" actions={<Link to="/translations">All translations →</Link>}>
        <Load query={recent}>
          {(page) => (
            <ul className="list">
              {page.items.map((t) => (
                <li key={t.id}>
                  <Link to={`/translations/${t.id}`} className="list-row">
                    <span className="list-main">
                      <strong>{t.show}</strong> <span className="muted">{episode(t)}</span>
                      <span className="list-sub">{dateTime(t.at)}</span>
                    </span>
                    <span className="list-tags">
                      <StepBadge step={t.step} />
                      <RepairBadge value={t.repair} />
                      {t.weak && <span className="badge tone-bad">weak</span>}
                      <span className="num">{euro(t.eur)}</span>
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Load>
      </Card>
    </>
  );
}

function Kpis({ s }: { s: Summary }) {
  const change = s.eur_prev_month > 0 ? `${euro(s.eur_prev_month)} last month` : "nothing last month";
  return (
    <div className="kpis">
      <Kpi label="Costs this month" value={euro(s.eur_month)} sub={`${change} · ${euro(s.eur_total)} in total`} />
      <Kpi
        label="Translations this month"
        value={number(s.translations_month)}
        sub={`${number(s.translations)} in total · last ${dateTime(s.last_translation_at)}`}
      />
      <Kpi
        label="Average quality score"
        value={number(s.avg_score, 1)}
        sub={`${number(s.judged)} judged by AI`}
        tone={scoreTone(s.avg_score)}
      />
      <Kpi
        label="Needed step 2"
        value={percent(s.fallback_pct)}
        sub={`${number(s.fallbacks)} of ${number(s.translations)}`}
        tone={(s.fallback_pct ?? 0) > 10 ? "warn" : "good"}
      />
      <Kpi
        label="Weak translations"
        value={number(s.weak)}
        sub="still failing a check"
        tone={s.weak > 0 ? "bad" : "good"}
      />
    </div>
  );
}

/** Stacked per model; days without calls are shown as empty. */
export function costChartData(rows: CostRow[], from: string, to: string) {
  const totals = new Map<string, number>();
  for (const r of rows) if (r.model) totals.set(r.model, (totals.get(r.model) ?? 0) + r.eur);
  const models = [...totals.keys()].sort((a, b) => (totals.get(b) ?? 0) - (totals.get(a) ?? 0));
  const byDay = new Map<string, Record<string, number | string>>(dayRange(from, to).map((day) => [day, { day }]));
  for (const r of rows) {
    const day = byDay.get(r.key);
    if (day && r.model) day[r.model] = r.eur;
  }
  return { models, data: [...byDay.values()] };
}

function CostChart({ rows, from }: { rows: CostRow[]; from: string }) {
  const { models, data } = costChartData(rows, from, isoDay(new Date()));
  if (models.length === 0) return <p className="muted">No calls in this period.</p>;
  return (
    <div className="chart">
      <ResponsiveContainer width="100%" height={260}>
        <BarChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid vertical={false} stroke="var(--grid)" />
          <XAxis dataKey="day" tickFormatter={shortDate} tick={{ fill: "var(--muted)", fontSize: 12 }} minTickGap={24} />
          <YAxis tickFormatter={(v: number) => euro(v)} tick={{ fill: "var(--muted)", fontSize: 12 }} width={72} />
          <Tooltip
            cursor={{ fill: "var(--hover)" }}
            contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 8 }}
            labelFormatter={(day) => shortDate(String(day))}
            formatter={(value, name) => [euro(Number(value)), model(String(name))]}
          />
          <Legend formatter={(name) => model(String(name))} />
          {models.map((m, i) => (
            <Bar key={m} dataKey={m} stackId="eur" fill={modelColor(m, i)} maxBarSize={28} isAnimationActive={false} />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

function ScoreChart({ s }: { s: Summary }) {
  if (s.judged === 0) return <p className="muted">No translations judged yet.</p>;
  const data = s.score_buckets.filter((b) => b.from >= 40 || b.count > 0);
  return (
    <div className="chart">
      <ResponsiveContainer width="100%" height={260}>
        <BarChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid vertical={false} stroke="var(--grid)" />
          <XAxis dataKey="from" tickFormatter={(v: number) => `${v}+`} tick={{ fill: "var(--muted)", fontSize: 12 }} />
          <YAxis allowDecimals={false} tick={{ fill: "var(--muted)", fontSize: 12 }} width={32} />
          <Tooltip
            cursor={{ fill: "var(--hover)" }}
            contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 8 }}
            labelFormatter={(from) => `Score ${from}–${Number(from) + 9}`}
            formatter={(value) => [value, "translations"]}
          />
          <Bar dataKey="count" fill="var(--blue)" maxBarSize={40} radius={[4, 4, 0, 0]} isAnimationActive={false} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
