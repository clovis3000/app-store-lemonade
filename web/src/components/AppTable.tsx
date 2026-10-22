import { Link } from 'react-router-dom';
import type { AppSummary } from '../api/types';
import ScoreBadge from './ScoreBadge';

interface Props {
  apps: AppSummary[];
  sort: string;
  order: string;
  onSort: (col: string) => void;
}

const columns = [
  { key: 'name', label: 'App' },
  { key: 'category', label: 'Category' },
  { key: 'rating', label: 'Rating' },
  { key: 'rating_count', label: 'Reviews' },
  { key: 'score', label: 'Score' },
  { key: 'last_updated', label: 'Updated' },
];

function stars(rating: number | null) {
  if (rating === null) return '--';
  return `${rating.toFixed(1)}`;
}

export default function AppTable({ apps, sort, order, onSort }: Props) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left">
            {columns.map((col) => {
              const active = sort === col.key;
              return (
                <th
                  key={col.key}
                  onClick={() => onSort(col.key)}
                  className="px-3 py-2 font-medium text-text-muted cursor-pointer hover:text-text select-none"
                >
                  {col.label}
                  {active && (
                    <span className="ml-1 text-blue-600">{order === 'asc' ? '\u2191' : '\u2193'}</span>
                  )}
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {apps.map((app) => (
            <tr key={app.app_id} className="border-b border-border-light hover:bg-surface-hover">
              <td className="px-3 py-2.5">
                <Link to={`/apps/${app.app_id}`} className="text-blue-600 dark:text-blue-400 hover:underline font-medium">
                  {app.starred && <span className="text-yellow-400 mr-1">{'\u2605'}</span>}{app.name}
                </Link>
                {app.developer && (
                  <span className="block text-xs text-text-faint">{app.developer}</span>
                )}
              </td>
              <td className="px-3 py-2.5 text-text-muted">{app.category ?? '--'}</td>
              <td className="px-3 py-2.5">{stars(app.rating)}</td>
              <td className="px-3 py-2.5 text-text-muted">{app.rating_count?.toLocaleString() ?? '--'}</td>
              <td className="px-3 py-2.5"><ScoreBadge score={app.opportunity_score} /></td>
              <td className="px-3 py-2.5 text-text-faint text-xs">
                {app.last_updated ? new Date(app.last_updated).toLocaleDateString() : '--'}
              </td>
            </tr>
          ))}
          {apps.length === 0 && (
            <tr>
              <td colSpan={6} className="px-3 py-8 text-center text-text-faint">No apps found</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
