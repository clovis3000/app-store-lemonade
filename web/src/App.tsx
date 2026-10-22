import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import DashboardPage from './pages/DashboardPage';
import AppsPage from './pages/AppsPage';
import AppDetailPage from './pages/AppDetailPage';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/apps" element={<AppsPage />} />
          <Route path="/apps/:appId" element={<AppDetailPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
