interface Props {
  score: number | null;
}

export default function ScoreBadge({ score }: Props) {
  if (score === null) return <span className="text-text-faint text-sm">--</span>;

  let bg = 'bg-surface-hover text-text-muted';
  if (score >= 2.0) bg = 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400';
  else if (score >= 1.0) bg = 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400';
  else if (score > 0) bg = 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400';

  return (
    <span className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${bg}`}>
      {score.toFixed(2)}
    </span>
  );
}
