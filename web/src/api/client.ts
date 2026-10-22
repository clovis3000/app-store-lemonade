import type {
  StatsResponse,
  PaginatedResponse,
  AppSummary,
  AppDetail,
  Review,
} from './types';

const BASE = '/api';

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new Error(`API ${res.status}: ${res.statusText}`);
  return res.json();
}

export function fetchStats(): Promise<StatsResponse> {
  return get('/stats');
}

export interface ListAppsParams {
  filter?: string;
  page?: number;
  per_page?: number;
  sort?: string;
  order?: string;
  category?: string;
  search?: string;
}

export function fetchApps(params: ListAppsParams = {}): Promise<PaginatedResponse<AppSummary>> {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== '') qs.set(k, String(v));
  }
  return get(`/apps?${qs}`);
}

export function fetchApp(appId: string): Promise<AppDetail> {
  return get(`/apps/${appId}`);
}

export interface ListReviewsParams {
  page?: number;
  per_page?: number;
  country?: string;
  rating?: number;
  sort?: string;
  order?: string;
}

export function fetchReviews(appId: string, params: ListReviewsParams = {}): Promise<PaginatedResponse<Review>> {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== '') qs.set(k, String(v));
  }
  return get(`/apps/${appId}/reviews?${qs}`);
}

export async function toggleStar(appId: string): Promise<{ starred: boolean }> {
  const res = await fetch(`${BASE}/apps/${appId}/star`, { method: 'POST' });
  if (!res.ok) throw new Error(`API ${res.status}: ${res.statusText}`);
  return res.json();
}
