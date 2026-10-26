import { useEffect, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";

import type { TranslationsQuery } from "../api/client";
import { useShows, useTranslations } from "../api/queries";
import { Badge, Card, Load, RepairBadge, StepBadge, scoreTone } from "../components/ui";
import { check, dateTime, episode, euro, number } from "../format";

type Sort = NonNullable<TranslationsQuery["sort"]>;
const PER_PAGE = 50;

/** Filters live in the URL, so a filtered view can be bookmarked and the back button works. */
function useFilters() {
  const [params, setParams] = useSearchParams();
  const query: TranslationsQuery = {
    show: params.get("show") || undefined,
    q: params.get("q") || undefined,
    weak: params.get("weak") === "true" ? true : undefined,
    step: params.get("step") ? Number(params.get("step")) : undefined,
    sort: (params.get("sort") as Sort | null) ?? undefined,
    order: params.get("order") === "asc" ? "asc" : params.get("order") === "desc" ? "desc" : undefined,
    page: Number(params.get("page") ?? 1) || 1,
    per_page: PER_PAGE,
  };
  const set = (changes: Record<string, string | undefined>, keepPage = false) => {
    const next = new URLSearchParams(params);
    for (const [key, value] of Object.entries(changes)) {
      if (value) next.set(key, value);
      else next.delete(key);
    }
    if (!keepPage) next.delete("page");
    setParams(next, { replace: true });
  };
  return { query, set };
}

export function Translations() {
  const { query, set } = useFilters();
  const page = useTranslations(query);
  const shows = useShows();
  const navigate = useNavigate();
  const [search, setSearch] = useState(query.q ?? "");
  // Follow the URL when it changes from outside the box (back button, a link).
  const [urlQ, setUrlQ] = useState(query.q);
  if (query.q !== urlQ) {
    setUrlQ(query.q);
    setSearch(query.q ?? "");
  }

  // Search as you type, a moment after the last key.
  useEffect(() => {
    if (search === (query.q ?? "")) return;
    const timer = setTimeout(() => set({ q: search.trim() || undefined }), 300);
    return () => clearTimeout(timer);
  });

  const sortBy = (sort: Sort) => {
    const current = query.sort ?? "at";
    const order = current === sort && (query.order ?? "desc") === "desc" ? "asc" : "desc";
    set({ sort, order });
  };
  const arrow = (sort: Sort) =>
    (query.sort ?? "at") === sort ? ((query.order ?? "desc") === "desc" ? " ↓" : " ↑") : "";

  return (
    <Card title="Translations">
      <div className="filters">
        <input
          type="search"
          placeholder="Search show, episode or file"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Search"
        />
        <select value={query.show ?? ""} onChange={(e) => set({ show: e.target.value || undefined })} aria-label="Show">
          <option value="">All shows</option>
          {shows.data?.map((s) => (
            <option key={s.show} value={s.show}>
              {s.show}
            </option>
          ))}
        </select>
        <select
          value={query.step ?? ""}
          onChange={(e) => set({ step: e.target.value || undefined })}
          aria-label="Step"
        >
          <option value="">Both steps</option>
          <option value="1">Step 1</option>
          <option value="2">Step 2</option>
        </select>
        <label className="toggle">
          <input
            type="checkbox"
            checked={query.weak === true}
            onChange={(e) => set({ weak: e.target.checked ? "true" : undefined })}
          />
          Weak only
        </label>
      </div>

      <Load query={page}>
        {(p) => (
          <>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th className="hide-sm">
                      <button onClick={() => sortBy("at")}>When{arrow("at")}</button>
                    </th>
                    <th>
                      <button onClick={() => sortBy("show")}>Show{arrow("show")}</button>
                    </th>
                    <th className="hide-sm">Step</th>
                    <th className="hide-sm">Checks</th>
                    <th className="num">
                      <button onClick={() => sortBy("score")}>Score{arrow("score")}</button>
                    </th>
                    <th className="num">
                      <button onClick={() => sortBy("eur")}>Cost{arrow("eur")}</button>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {p.items.map((t) => (
                    <tr key={t.id} className="clickable" onClick={() => navigate(`/translations/${t.id}`)}>
                      <td className="nowrap muted hide-sm">{dateTime(t.at)}</td>
                      <td>
                        <Link to={`/translations/${t.id}`} onClick={(e) => e.stopPropagation()}>
                          {t.show}
                        </Link>{" "}
                        <span className="muted">
                          {episode(t)} {t.episode_title}
                        </span>
                        <span className="tags show-sm">
                          <span className="muted small">{dateTime(t.at)}</span>
                          {t.step === 2 && <StepBadge step={t.step} />}
                          {t.weak && <Badge tone="bad">weak</Badge>}
                          <RepairBadge value={t.repair} />
                        </span>
                      </td>
                      <td className="hide-sm">
                        <StepBadge step={t.step} />
                      </td>
                      <td className="hide-sm">
                        <span className="tags">
                          {t.weak && <Badge tone="bad">weak</Badge>}
                          <RepairBadge value={t.repair} />
                          {t.step1_failed.map((c) => (
                            <span key={c} className="muted small">
                              {check(c)}
                            </span>
                          ))}
                        </span>
                      </td>
                      <td className={`num tone-text-${scoreTone(t.score) ?? "none"}`}>{number(t.score)}</td>
                      <td className="num">{euro(t.eur)}</td>
                    </tr>
                  ))}
                  {p.items.length === 0 && (
                    <tr>
                      <td colSpan={6} className="muted">
                        No translations match.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
            <Pager total={p.total} page={p.page} perPage={p.per_page} go={(n) => set({ page: String(n) }, true)} />
          </>
        )}
      </Load>
    </Card>
  );
}

function Pager({ total, page, perPage, go }: { total: number; page: number; perPage: number; go: (n: number) => void }) {
  const pages = Math.max(1, Math.ceil(total / perPage));
  return (
    <div className="pager">
      <span className="muted">
        {number(total)} translation{total === 1 ? "" : "s"}
      </span>
      {pages > 1 && (
        <span>
          <button disabled={page <= 1} onClick={() => go(page - 1)}>
            ← Previous
          </button>
          <span className="muted">
            {page} / {pages}
          </span>
          <button disabled={page >= pages} onClick={() => go(page + 1)}>
            Next →
          </button>
        </span>
      )}
    </div>
  );
}
