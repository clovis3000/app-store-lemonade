interface Props {
  label: string;
  value: string | number;
}

export default function StatCard({ label, value }: Props) {
  return (
    <div className="bg-surface rounded-lg border border-border p-5">
      <p className="text-sm text-text-muted mb-1">{label}</p>
      <p className="text-2xl font-semibold">{typeof value === 'number' ? value.toLocaleString() : value}</p>
    </div>
  );
}
