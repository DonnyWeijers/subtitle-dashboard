import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { api, unwrap, type CostsQuery, type TranslationsQuery } from "./client";

// subtrans translates a few files an hour: polling once a minute is plenty.
const live = { refetchInterval: 60_000 };

export const useSummary = (month?: string) =>
  useQuery({
    queryKey: ["summary", month],
    queryFn: () => unwrap(api.GET("/api/summary", { params: { query: { month } } })),
    ...live,
  });

export const useCosts = (query: CostsQuery) =>
  useQuery({
    queryKey: ["costs", query],
    queryFn: () => unwrap(api.GET("/api/costs", { params: { query } })),
    ...live,
  });

export const useTranslations = (query: TranslationsQuery) =>
  useQuery({
    queryKey: ["translations", query],
    queryFn: () => unwrap(api.GET("/api/translations", { params: { query } })),
    placeholderData: keepPreviousData,
    ...live,
  });

export const useTranslation = (id: string) =>
  useQuery({
    queryKey: ["translation", id],
    queryFn: () => unwrap(api.GET("/api/translations/{id}", { params: { path: { id } } })),
  });

export const useChecks = (days: number) =>
  useQuery({
    queryKey: ["checks", days],
    queryFn: () => unwrap(api.GET("/api/checks", { params: { query: { days } } })),
    ...live,
  });

export const useShows = () =>
  useQuery({ queryKey: ["shows"], queryFn: () => unwrap(api.GET("/api/shows")), ...live });
