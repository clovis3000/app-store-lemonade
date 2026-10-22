import { useEffect, useState } from 'react';
import { fetchApps, fetchStats } from '../api/client';
import type { PaginatedResponse, AppSummary } from '../api/types';
import FilterBar from '../components/FilterBar';
import AppTable from '../components/AppTable';
import Pagination from '../components/Pagination';

export default function AppsPage() {
  const [data, setData] = useState<PaginatedResponse<AppSummary> | null>(null);
  const [categories, setCategories] = useState<string[]>([]);
  const [filter, setFilter] = useState('scored');
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('');
  const [sort, setSort] = useState('score');
  const [order, setOrder] = useState('desc');
  const [page, setPage] = useState(1);

  useEffect(() => {
    fetchStats().then((s) => {
      setCategories(s.categories.map((c) => c.category));
    });
  }, []);

  useEffect(() => {
    fetchApps({ filter, search, category, sort, order, page, per_page: 50 }).then(setData);
  }, [filter, search, category, sort, order, page]);

  const handleSort = (col: string) => {
    if (sort === col) {
      setOrder(order === 'desc' ? 'asc' : 'desc');
    } else {
      setSort(col);
      setOrder('desc');
    }
    setPage(1);
  };

  return (
    <div>
      <h1 className="text-xl font-semibold mb-4">Apps</h1>
      <FilterBar
        filter={filter}
        search={search}
        category={category}
        categories={categories}
        onFilterChange={(f) => { setFilter(f); setPage(1); }}
        onSearchChange={(s) => { setSearch(s); setPage(1); }}
        onCategoryChange={(c) => { setCategory(c); setPage(1); }}
      />
      {data ? (
        <>
          <div className="bg-surface border border-border rounded-lg overflow-hidden">
            <AppTable apps={data.data} sort={sort} order={order} onSort={handleSort} />
          </div>
          <Pagination pagination={data.pagination} onPageChange={setPage} />
        </>
      ) : (
        <p className="text-text-faint py-8">Loading...</p>
      )}
    </div>
  );
}
