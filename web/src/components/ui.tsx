import type { UseQueryResult } from "@tanstack/react-query";
import type { ReactNode } from "react";

import type { Repair } from "../api/client";
import { repair } from "../format";

export type Tone = "good" | "warn" | "bad" | "neutral";

export function Card({ title, actions, children }: { title?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="card">
      {(title || actions) && (
        <header className="card-head">
          {title && <h2>{title}</h2>}
          {actions}
        </header>
      )}
      {children}
    </section>
  );
}

export function Kpi({ label, value, sub, tone }: { label: string; value: string; sub?: string; tone?: Tone }) {
  return (
    <div className={`kpi ${tone ? `tone-${tone}` : ""}`}>
      <span className="kpi-label">{label}</span>
      <span className="kpi-value">{value}</span>
      {sub && <span className="kpi-sub">{sub}</span>}
    </div>
  );
}

export function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return <span className={`badge tone-${tone}`}>{children}</span>;
}

export function StepBadge({ step }: { step: number }) {
  return <Badge tone={step === 1 ? "neutral" : "warn"}>step {step}</Badge>;
}

export function RepairBadge({ value }: { value: Repair | null | undefined }) {
  if (!value) return null;
  return <Badge tone={value === "unfixed" ? "bad" : value === "step1-batches" ? "good" : "warn"}>{repair(value)}</Badge>;
}

/** 80 on the judge's scale reads as good, below 70 as weak (write-up 48: 72 on this judge ≈ "80"). */
export function scoreTone(score: number | null | undefined): Tone | undefined {
  if (score == null) return undefined;
  return score >= 80 ? "good" : score >= 70 ? "warn" : "bad";
}

/** Loading and error states for a query, then its data. */
export function Load<T>({ query, children }: { query: UseQueryResult<T>; children: (data: T) => ReactNode }) {
  if (query.isPending) return <p className="muted">Loading…</p>;
  if (query.isError) return <p className="error">Could not load: {query.error.message}</p>;
  return <>{children(query.data)}</>;
}

const MODEL_COLORS: [string, string][] = [
  ["luna", "var(--blue)"],
  ["gemini", "var(--purple)"],
  ["claude", "var(--teal)"],
  ["sonnet", "var(--teal)"],
];
const FALLBACK_COLORS = ["var(--pink)", "var(--yellow)", "var(--indigo)", "var(--grey)"];

export function modelColor(name: string, index: number): string {
  const known = MODEL_COLORS.find(([part]) => name.toLowerCase().includes(part));
  return known ? known[1] : FALLBACK_COLORS[index % FALLBACK_COLORS.length]!;
}
