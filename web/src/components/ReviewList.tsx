import { useState, useEffect } from 'react';
import { fetchReviews } from '../api/client';
import type { Review, PaginatedResponse } from '../api/types';
import Pagination from './Pagination';

interface Props {
  appId: string;
  countries: string[];
}

const ratingStars = (n: number) => '\u2605'.repeat(n) + '\u2606'.repeat(5 - n);

export default function ReviewList({ appId, countries }: Props) {
  const [data, setData] = useState<PaginatedResponse<Review> | null>(null);
  const [page, setPage] = useState(1);
  const [country, setCountry] = useState('');
  const [rating, setRating] = useState('');

  useEffect(() => {
    fetchReviews(appId, {
      page,
      per_page: 20,
      country: country || undefined,
      rating: rating ? Number(rating) : undefined,
    }).then(setData);
  }, [appId, page, country, rating]);

  return (
    <div>
      <div className="flex gap-3 mb-4">
        <select
          value={country}
          onChange={(e) => { setCountry(e.target.value); setPage(1); }}
          className="px-2 py-1 text-sm border border-border rounded bg-surface"
        >
          <option value="">All countries</option>
          {countries.map((c) => (
            <option key={c} value={c}>{c.toUpperCase()}</option>
          ))}
        </select>
        <select
          value={rating}
          onChange={(e) => { setRating(e.target.value); setPage(1); }}
          className="px-2 py-1 text-sm border border-border rounded bg-surface"
        >
          <option value="">All ratings</option>
          {[1, 2, 3, 4, 5].map((r) => (
            <option key={r} value={r}>{r} star{r > 1 ? 's' : ''}</option>
          ))}
        </select>
      </div>
      {data && (
        <>
          <div className="space-y-3">
            {data.data.map((r) => (
              <div key={r.review_id} className="bg-surface border border-border rounded-lg p-4">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-yellow-500 text-sm">{ratingStars(r.rating)}</span>
                  <span className="font-medium text-sm">{r.title || '(no title)'}</span>
                </div>
                {r.body && <p className="text-sm text-text-muted mb-2">{r.body}</p>}
                <div className="flex gap-3 text-xs text-text-faint">
                  {r.author && <span>{r.author}</span>}
                  {r.country && <span>{r.country.toUpperCase()}</span>}
                  {r.date && <span>{new Date(r.date).toLocaleDateString()}</span>}
                  {r.version && <span>v{r.version}</span>}
                </div>
              </div>
            ))}
            {data.data.length === 0 && (
              <p className="text-center text-text-faint py-8">No reviews found</p>
            )}
          </div>
          <Pagination pagination={data.pagination} onPageChange={setPage} />
        </>
      )}
    </div>
  );
}
