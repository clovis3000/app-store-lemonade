import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { fetchStats } from '../api/client';
import type { StatsResponse } from '../api/types';
import StatCard from '../components/StatCard';
import ScoreBadge from '../components/ScoreBadge';

export default function DashboardPage() {
  const [stats, setStats] = useState<StatsResponse | null>(null);

  useEffect(() => {
    fetchStats().then(setStats);
  }, []);

  if (!stats) return <p className="text-text-faint py-8">Loading...</p>;

  const { pipeline, categories, score_distribution, top_opportunities } = stats;

  return (
    <div className="space-y-8">
      <h1 className="text-xl font-semibold">Dashboard</h1>

      {/* Pipeline Funnel */}
      <div className="grid grid-cols-3 sm:grid-cols-6 gap-4">
        <StatCard label="Total Apps" value={pipeline.total_apps} />
        <StatCard label="Scored" value={pipeline.scored} />
        <StatCard label="Diagnosed" value={pipeline.diagnosed} />
        <StatCard label="Actionable" value={pipeline.actionable} />
        <StatCard label="Starred" value={pipeline.starred} />
        <StatCard label="Total Reviews" value={pipeline.total_reviews} />
      </div>

      {/* Score Distribution */}
      <section>
        <h2 className="font-semibold mb-3">Score Distribution</h2>
        <div className="flex items-end gap-2 h-32">
          {score_distribution.map((b) => {
            const maxCount = Math.max(...score_distribution.map((x) => x.count));
            const pct = maxCount > 0 ? (b.count / maxCount) * 100 : 0;
            return (
              <div key={b.range} className="flex-1 flex flex-col items-center">
                <span className="text-xs text-text-muted mb-1">{b.count}</span>
                <div
                  className="w-full bg-blue-400 rounded-t"
                  style={{ height: `${Math.max(pct, 2)}%` }}
                />
                <span className="text-xs text-text-faint mt-1">{b.range}</span>
              </div>
            );
          })}
        </div>
      </section>

      <div className="grid grid-cols-2 gap-8">
        {/* Top Opportunities */}
        <section>
          <h2 className="font-semibold mb-3">Top Opportunities</h2>
          <div className="bg-surface border border-border rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left">
                  <th className="px-3 py-2 text-text-muted font-medium">App</th>
                  <th className="px-3 py-2 text-text-muted font-medium">Category</th>
                  <th className="px-3 py-2 text-text-muted font-medium">Score</th>
                </tr>
              </thead>
              <tbody>
                {top_opportunities.map((app) => (
                  <tr key={app.app_id} className="border-b border-border-light hover:bg-surface-hover">
                    <td className="px-3 py-2">
                      <Link to={`/apps/${app.app_id}`} className="text-blue-600 dark:text-blue-400 hover:underline">
                        {app.name}
                      </Link>
                    </td>
                    <td className="px-3 py-2 text-text-muted">{app.category ?? '--'}</td>
                    <td className="px-3 py-2"><ScoreBadge score={app.opportunity_score} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        {/* Category Breakdown */}
        <section>
          <h2 className="font-semibold mb-3">Categories</h2>
          <div className="bg-surface border border-border rounded-lg overflow-hidden max-h-96 overflow-y-auto">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-surface">
                <tr className="border-b border-border text-left">
                  <th className="px-3 py-2 text-text-muted font-medium">Category</th>
                  <th className="px-3 py-2 text-text-muted font-medium text-right">Apps</th>
                  <th className="px-3 py-2 text-text-muted font-medium text-right">Avg Rating</th>
                  <th className="px-3 py-2 text-text-muted font-medium text-right">Scored</th>
                </tr>
              </thead>
              <tbody>
                {categories.map((c) => (
                  <tr key={c.category} className="border-b border-border-light">
                    <td className="px-3 py-1.5">{c.category}</td>
                    <td className="px-3 py-1.5 text-right text-text-muted">{c.count.toLocaleString()}</td>
                    <td className="px-3 py-1.5 text-right text-text-muted">{c.avg_rating.toFixed(1)}</td>
                    <td className="px-3 py-1.5 text-right text-text-muted">{c.scored_count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      </div>
    </div>
  );
}
