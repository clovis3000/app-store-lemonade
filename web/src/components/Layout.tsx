import { NavLink, Outlet } from 'react-router-dom';
import ThemeToggle from './ThemeToggle';

const navItems = [
  { to: '/', label: 'Dashboard' },
  { to: '/apps', label: 'Apps' },
];

export default function Layout() {
  return (
    <div className="min-h-screen">
      <nav className="bg-surface border-b border-border px-6 py-3 flex items-center gap-8">
        <span className="font-semibold text-lg tracking-tight">lemonade</span>
        <div className="flex gap-1 flex-1">
          {navItems.map(({ to, label }) => (
            <NavLink
              key={to}
              to={to}
              end={to === '/'}
              className={({ isActive }) =>
                `px-3 py-1.5 rounded text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-blue-50 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300'
                    : 'text-text-muted hover:text-text hover:bg-surface-hover'
                }`
              }
            >
              {label}
            </NavLink>
          ))}
        </div>
        <ThemeToggle />
      </nav>
      <main className="max-w-7xl mx-auto px-6 py-6">
        <Outlet />
      </main>
    </div>
  );
}
