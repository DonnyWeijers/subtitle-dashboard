import type { Repair, TranslationRow } from "./api/client";

const LOCALE = "nl-NL";

/** Euros with enough decimals to tell small amounts apart: € 2,55 · € 0,118 · € 0,0084. */
export function euro(value: number | null | undefined): string {
  if (value == null) return "–";
  const abs = Math.abs(value);
  const digits = abs === 0 || abs >= 1 ? 2 : abs >= 0.1 ? 3 : 4;
  return new Intl.NumberFormat(LOCALE, {
    style: "currency",
    currency: "EUR",
    minimumFractionDigits: 2,
    maximumFractionDigits: digits,
  }).format(value);
}

export function number(value: number | null | undefined, decimals = 0): string {
  if (value == null) return "–";
  return new Intl.NumberFormat(LOCALE, { minimumFractionDigits: decimals, maximumFractionDigits: decimals }).format(
    value,
  );
}

export function percent(value: number | null | undefined, decimals = 1): string {
  return value == null ? "–" : `${number(value, decimals)}%`;
}

/** subtrans writes local time without a zone: 2026-10-23T14:20:52. */
export function dateTime(at: string | null | undefined): string {
  if (!at) return "–";
  const d = new Date(at);
  if (Number.isNaN(d.getTime())) return at;
  return new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }).format(d);
}

export function shortDate(day: string): string {
  const d = new Date(`${day}T12:00:00`);
  return Number.isNaN(d.getTime()) ? day : new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" }).format(d);
}

export function episode(row: Pick<TranslationRow, "season" | "episode">): string {
  if (row.season == null || row.episode == null) return "";
  return `S${String(row.season).padStart(2, "0")}E${String(row.episode).padStart(2, "0")}`;
}

/** openai/gpt-6-luna → gpt-6-luna */
export function model(name: string | null | undefined): string {
  if (!name) return "–";
  return name.includes("/") ? name.slice(name.indexOf("/") + 1) : name;
}

const CHECKS: Record<string, string> = {
  missing: "Missing lines",
  untranslated: "English left in",
  odd_length: "Odd line length",
  shifted: "Shifted lines",
};

export function check(name: string): string {
  return CHECKS[name] ?? name.replaceAll("_", " ");
}

const REPAIRS: Record<Repair, string> = {
  "step1-batches": "Batches redone, step 1",
  "step2-batches": "Batches redone, step 2",
  "step2-file": "Whole file, step 2",
  unfixed: "Not fixed",
};

export function repair(value: Repair | null | undefined): string {
  return value ? REPAIRS[value] : "–";
}

/** Local YYYY-MM-DD, `days` ago. */
export function daysAgo(days: number, from = new Date()): string {
  const d = new Date(from);
  d.setDate(d.getDate() - days);
  return isoDay(d);
}

export function isoDay(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Every day from `from` to `to`, inclusive. */
export function dayRange(from: string, to: string): string[] {
  const days: string[] = [];
  const d = new Date(`${from}T12:00:00`);
  const end = new Date(`${to}T12:00:00`);
  while (d <= end && days.length < 3700) {
    days.push(isoDay(d));
    d.setDate(d.getDate() + 1);
  }
  return days;
}
