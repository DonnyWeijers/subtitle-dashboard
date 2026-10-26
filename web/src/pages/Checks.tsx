import { useState } from "react";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

import type { Checks as ChecksData } from "../api/client";
import { useChecks } from "../api/queries";
import { Card, Kpi, Load, RepairBadge } from "../components/ui";
import { check, number, percent } from "../format";

const WINDOWS = [7, 30, 90, 365];

export function Checks() {
  const [days, setDays] = useState(30);
  const query = useChecks(days);
  return (
    <>
      <Card
        title="Free checks on step 1"
        actions={
          <select value={days} onChange={(e) => setDays(Number(e.target.value))} aria-label="Period">
            {WINDOWS.map((d) => (
              <option key={d} value={d}>
                Last {d} days
              </option>
            ))}
          </select>
        }
      >
        <p className="muted">
          Every translation gets four free checks: missing lines, English left in, odd line lengths and lines shifted
          onto their neighbours. When step 1 fails one, subtrans repairs it: first the failing batches with step 1's
          model, then those batches with step 2's model, and only then the whole file with step 2.
        </p>
      </Card>
      <Load query={query}>{(c) => <View c={c} />}</Load>
    </>
  );
}

function View({ c }: { c: ChecksData }) {
  const weeks = c.by_week.map((w) => ({ ...w, pct: w.checked ? (100 * w.failed) / w.checked : 0 }));
  return (
    <>
      <div className="kpis">
        <Kpi label="Checked" value={number(c.checked)} sub={`translations in ${c.days} days`} />
        <Kpi
          label="Failed at step 1"
          value={percent(c.fail_pct)}
          sub={`${number(c.failed)} repaired or tried`}
          tone={(c.fail_pct ?? 0) > 15 ? "warn" : "good"}
        />
        <Kpi
          label="Still weak"
          value={number(c.still_weak)}
          sub="failing after every repair"
          tone={c.still_weak > 0 ? "bad" : "good"}
        />
      </div>

      <div className="grid-2">
        <Card title="Which check failed">
          {c.by_check.length === 0 ? (
            <p className="muted">Nothing failed.</p>
          ) : (
            <ul className="bars">
              {c.by_check.map((b) => (
                <li key={b.check}>
                  <span>{check(b.check)}</span>
                  <span className="bar">
                    <span style={{ width: `${(100 * b.failed) / Math.max(1, c.failed)}%` }} />
                  </span>
                  <span className="num">{number(b.failed)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
        <Card title="How it was repaired">
          {c.by_repair.length === 0 ? (
            <p className="muted">Nothing to repair.</p>
          ) : (
            <ul className="list">
              {c.by_repair.map((r) => (
                <li key={r.repair} className="list-row">
                  <RepairBadge value={r.repair} />
                  <span className="num">{number(r.count)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      {weeks.length > 0 && (
        <Card title="Share failing at step 1, per week">
          <div className="chart">
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={weeks} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                <CartesianGrid vertical={false} stroke="var(--grid)" />
                <XAxis dataKey="week" tick={{ fill: "var(--muted)", fontSize: 12 }} />
                <YAxis tickFormatter={(v: number) => `${v}%`} tick={{ fill: "var(--muted)", fontSize: 12 }} width={44} />
                <Tooltip
                  cursor={{ fill: "var(--hover)" }}
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 8 }}
                  formatter={(value, _name, item) => [
                    `${percent(Number(value))} (${item.payload.failed} of ${item.payload.checked})`,
                    "failed",
                  ]}
                />
                <Bar dataKey="pct" fill="var(--orange)" maxBarSize={40} radius={[4, 4, 0, 0]} isAnimationActive={false} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </Card>
      )}
    </>
  );
}
