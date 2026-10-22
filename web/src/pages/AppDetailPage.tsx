import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { fetchApp, toggleStar } from '../api/client';
import type { AppDetail } from '../api/types';
import ScoreBadge from '../components/ScoreBadge';
import DiagnosisCard from '../components/DiagnosisCard';
import ReviewList from '../components/ReviewList';

type Tab = 'overview' | 'diagnosis' | 'reviews';

export default function AppDetailPage() {
  const { appId } = useParams<{ appId: string }>();
  const [app, setApp] = useState<AppDetail | null>(null);
  const [starred, setStarred] = useState(false);
  const [tab, setTab] = useState<Tab>('overview');

  useEffect(() => {
    if (appId) fetchApp(appId).then((a) => { setApp(a); setStarred(a.starred); });
  }, [appId]);

  const handleToggleStar = async () => {
    if (!appId) return;
    const res = await toggleStar(appId);
    setStarred(res.starred);
  };

  if (!app) return <p className="text-text-faint py-8">Loading...</p>;

  const countries = Object.keys(app.review_summary.by_country);
  const fileSizeMB = app.file_size_bytes ? (app.file_size_bytes / 1_000_000).toFixed(1) : null;

  const tabs: { key: Tab; label: string }[] = [
    { key: 'overview', label: 'Overview' },
    { key: 'diagnosis', label: 'Diagnosis' },
    { key: 'reviews', label: `Reviews (${app.review_count})` },
  ];

  return (
    <div>
      <Link to="/apps" className="text-sm text-text-muted hover:text-text mb-4 inline-block">&larr; Back to apps</Link>

      {/* Header */}
      <div className="bg-surface border border-border rounded-lg p-6 mb-6">
        <div className="flex items-start justify-between">
          <div>
            <h1 className="text-2xl font-semibold">{app.name}</h1>
            <p className="text-text-muted">{app.developer ?? 'Unknown developer'}</p>
            <div className="flex items-center gap-4 mt-2 text-sm text-text-muted">
              {app.category && <span>{app.category}</span>}
              {app.formatted_price && <span>{app.formatted_price}</span>}
              {app.version && <span>v{app.version}</span>}
              {fileSizeMB && <span>{fileSizeMB} MB</span>}
            </div>
          </div>
          <div className="text-right flex flex-col items-end gap-2">
            <div className="flex items-center gap-2">
              <button
                onClick={handleToggleStar}
                className="text-2xl leading-none transition-colors"
                title={starred ? 'Unstar' : 'Star'}
              >
                <span className={starred ? 'text-yellow-400' : 'text-text-faint hover:text-yellow-400'}>{starred ? '\u2605' : '\u2606'}</span>
              </button>
              <ScoreBadge score={app.opportunity_score} />
            </div>
            {app.rating !== null && (
              <p className="text-sm text-text-muted">{app.rating.toFixed(1)} ({app.rating_count?.toLocaleString()} ratings)</p>
            )}
          </div>
        </div>

        {/* Signal breakdown */}
        {app.opportunity_score !== null && (
          <div className="grid grid-cols-3 gap-4 mt-4 pt-4 border-t border-border-light">
            <div>
              <p className="text-xs text-text-faint">Demand</p>
              <p className="font-semibold">{app.demand_signal?.toFixed(2) ?? '--'}</p>
            </div>
            <div>
              <p className="text-xs text-text-faint">Dissatisfaction</p>
              <p className="font-semibold">{app.dissatisfaction_signal?.toFixed(2) ?? '--'}</p>
            </div>
            <div>
              <p className="text-xs text-text-faint">Neglect</p>
              <p className="font-semibold">{app.neglect_signal?.toFixed(2) ?? '--'}</p>
            </div>
          </div>
        )}
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 border-b border-border">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors ${
              tab === t.key
                ? 'border-blue-600 text-blue-600 dark:border-blue-400 dark:text-blue-400'
                : 'border-transparent text-text-muted hover:text-text'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {tab === 'overview' && (
        <div className="space-y-6">
          {app.description && (
            <section>
              <h2 className="font-semibold mb-2">Description</h2>
              <p className="text-sm text-text-muted whitespace-pre-line max-h-60 overflow-y-auto">{app.description}</p>
            </section>
          )}
          {app.app_store_url && (
            <a href={app.app_store_url} target="_blank" rel="noopener noreferrer" className="text-sm text-blue-600 dark:text-blue-400 hover:underline">
              View on App Store
            </a>
          )}
          {app.review_summary.total > 0 && (
            <section>
              <h2 className="font-semibold mb-2">Review Breakdown</h2>
              <div className="grid grid-cols-2 gap-6">
                <div>
                  <p className="text-sm text-text-muted mb-2">By Rating</p>
                  {[5, 4, 3, 2, 1].map((r) => {
                    const count = app.review_summary.by_rating[String(r)] ?? 0;
                    const pct = app.review_summary.total > 0 ? (count / app.review_summary.total) * 100 : 0;
                    return (
                      <div key={r} className="flex items-center gap-2 mb-1">
                        <span className="text-xs w-12 text-text-muted">{r} star</span>
                        <div className="flex-1 bg-surface-hover rounded h-2">
                          <div className="bg-yellow-400 rounded h-2" style={{ width: `${pct}%` }} />
                        </div>
                        <span className="text-xs text-text-faint w-10 text-right">{count}</span>
                      </div>
                    );
                  })}
                </div>
                <div>
                  <p className="text-sm text-text-muted mb-2">By Country</p>
                  {Object.entries(app.review_summary.by_country)
                    .sort(([, a], [, b]) => b - a)
                    .map(([country, count]) => (
                      <div key={country} className="flex justify-between text-sm mb-0.5">
                        <span className="text-text-muted">{country.toUpperCase()}</span>
                        <span className="text-text-faint">{count}</span>
                      </div>
                    ))}
                </div>
              </div>
            </section>
          )}
        </div>
      )}

      {tab === 'diagnosis' && (
        app.diagnosis ? (
          <DiagnosisCard diagnosis={app.diagnosis} />
        ) : (
          <p className="text-text-faint py-8 text-center">No diagnosis available. Run <code className="bg-surface-hover px-1 rounded">lemonade diagnose --app-id {app.app_id}</code> to generate one.</p>
        )
      )}

      {tab === 'reviews' && (
        <ReviewList appId={app.app_id} countries={countries} />
      )}
    </div>
  );
}
