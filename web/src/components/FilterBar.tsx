import { useEffect, useState } from 'react';

interface Props {
  filter: string;
  search: string;
  category: string;
  categories: string[];
  onFilterChange: (f: string) => void;
  onSearchChange: (s: string) => void;
  onCategoryChange: (c: string) => void;
}

const filters = [
  { value: 'all', label: 'All' },
  { value: 'starred', label: 'Starred' },
  { value: 'scored', label: 'Scored' },
  { value: 'diagnosed', label: 'Diagnosed' },
  { value: 'actionable', label: 'Actionable' },
];

export default function FilterBar({
  filter, search, category, categories,
  onFilterChange, onSearchChange, onCategoryChange,
}: Props) {
  const [localSearch, setLocalSearch] = useState(search);

  useEffect(() => { setLocalSearch(search); }, [search]);

  return (
    <div className="flex flex-wrap items-center gap-3 mb-4">
      <div className="flex gap-1">
        {filters.map((f) => (
          <button
            key={f.value}
            onClick={() => onFilterChange(f.value)}
            className={`px-3 py-1.5 text-sm rounded font-medium transition-colors ${
              filter === f.value
                ? 'bg-blue-50 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300'
                : 'text-text-muted hover:bg-surface-hover'
            }`}
          >
            {f.label}
          </button>
        ))}
      </div>
      <input
        type="text"
        placeholder="Search apps..."
        value={localSearch}
        onChange={(e) => setLocalSearch(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && onSearchChange(localSearch)}
        onBlur={() => onSearchChange(localSearch)}
        className="px-3 py-1.5 text-sm border border-border rounded w-56 bg-surface focus:outline-none focus:ring-1 focus:ring-blue-400"
      />
      <select
        value={category}
        onChange={(e) => onCategoryChange(e.target.value)}
        className="px-3 py-1.5 text-sm border border-border rounded bg-surface focus:outline-none focus:ring-1 focus:ring-blue-400"
      >
        <option value="">All categories</option>
        {categories.map((c) => (
          <option key={c} value={c}>{c}</option>
        ))}
      </select>
    </div>
  );
}
