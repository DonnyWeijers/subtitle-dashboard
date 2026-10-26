import createClient from "openapi-fetch";

import type { components, paths } from "./schema";

/** Typed against the Rust API's OpenAPI spec (`scripts/gen-api.sh`). */
export const api = createClient<paths>({
  baseUrl: window.location.origin,
  // Look fetch up per call (not once at import), so tests can stub it.
  fetch: (request) => globalThis.fetch(request),
});

type Schemas = components["schemas"];
export type Summary = Schemas["Summary"];
export type CostRow = Schemas["CostRow"];
export type TranslationRow = Schemas["TranslationRow"];
export type TranslationPage = Schemas["TranslationPage"];
export type TranslationDetail = Schemas["TranslationDetail"];
export type Checks = Schemas["Checks"];
export type ShowRow = Schemas["ShowRow"];
export type Repair = Schemas["Repair"];

export type CostsQuery = NonNullable<paths["/api/costs"]["get"]["parameters"]["query"]>;
export type TranslationsQuery = NonNullable<paths["/api/translations"]["get"]["parameters"]["query"]>;

/** openapi-fetch returns `{ data, error }`; React Query wants a value or a thrown error. */
export async function unwrap<T>(request: Promise<{ data?: T; error?: unknown; response: Response }>): Promise<T> {
  const { data, error, response } = await request;
  if (data === undefined) {
    const detail = error && typeof error === "object" && "error" in error ? `: ${String(error.error)}` : "";
    throw new Error(`${response.status} ${response.statusText}${detail}`);
  }
  return data;
}
