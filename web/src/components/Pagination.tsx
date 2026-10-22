import type { Pagination as PaginationType } from '../api/types';

interface Props {
  pagination: PaginationType;
  onPageChange: (page: number) => void;
}

export default function Pagination({ pagination, onPageChange }: Props) {
  const { page, total_pages, total } = pagination;
  if (total_pages <= 1) return null;

  const pages: number[] = [];
  const start = Math.max(1, page - 2);
  const end = Math.min(total_pages, page + 2);
  for (let i = start; i <= end; i++) pages.push(i);

  return (
    <div className="flex items-center justify-between pt-4">
      <p className="text-sm text-text-muted">{total.toLocaleString()} results</p>
      <div className="flex gap-1">
        <button
          onClick={() => onPageChange(page - 1)}
          disabled={page <= 1}
          className="px-2.5 py-1 text-sm rounded border border-border disabled:opacity-40 hover:bg-surface-hover"
        >
          Prev
        </button>
        {start > 1 && <span className="px-1 py-1 text-sm text-text-faint">...</span>}
        {pages.map((p) => (
          <button
            key={p}
            onClick={() => onPageChange(p)}
            className={`px-2.5 py-1 text-sm rounded border ${
              p === page
                ? 'bg-blue-50 border-blue-300 text-blue-700 font-medium dark:bg-blue-900/40 dark:border-blue-700 dark:text-blue-300'
                : 'border-border hover:bg-surface-hover'
            }`}
          >
            {p}
          </button>
        ))}
        {end < total_pages && <span className="px-1 py-1 text-sm text-text-faint">...</span>}
        <button
          onClick={() => onPageChange(page + 1)}
          disabled={page >= total_pages}
          className="px-2.5 py-1 text-sm rounded border border-border disabled:opacity-40 hover:bg-surface-hover"
        >
          Next
        </button>
      </div>
    </div>
  );
}
