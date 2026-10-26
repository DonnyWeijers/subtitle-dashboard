import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ShowRow } from "../api/client";
import { Shows } from "./Shows";

const rows: ShowRow[] = [
  {
    show: "Harbour Watch",
    show_kind: "series",
    translations: 3,
    eur: 0.16684,
    eur_spent: 0.17028,
    avg_score: 71,
    judged: 2,
    weak: 1,
    fallbacks: 2,
    last_at: "2026-10-05T08:30:00",
  },
];

function renderWith(response: Response) {
  vi.stubGlobal("fetch", vi.fn(async () => response));
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Shows />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Shows", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("lists shows with a link to their translations", async () => {
    renderWith(new Response(JSON.stringify(rows), { headers: { "Content-Type": "application/json" } }));
    const link = await screen.findByRole("link", { name: "Harbour Watch" });
    expect(link.getAttribute("href")).toBe("/translations?show=Harbour%20Watch");
    expect(screen.getByText(/0,17/)).toBeTruthy();
  });

  it("shows the API's error", async () => {
    renderWith(new Response(JSON.stringify({ error: "internal error" }), { status: 500, statusText: "Internal Server Error" }));
    expect(await screen.findByText(/Could not load: 500 Internal Server Error: internal error/)).toBeTruthy();
  });
});
