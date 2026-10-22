import type { Diagnosis } from '../api/types';

interface Props {
  diagnosis: Diagnosis;
}

export default function DiagnosisCard({ diagnosis }: Props) {
  return (
    <div className="space-y-6">
      {/* Build Brief */}
      {diagnosis.build_brief && (
        <section>
          <h3 className="font-semibold mb-2">Build Brief</h3>
          <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4 space-y-3">
            <p className="font-medium">{diagnosis.build_brief.one_liner}</p>
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <p className="text-text-muted mb-1">Core Features</p>
                <ul className="list-disc list-inside space-y-0.5">
                  {diagnosis.build_brief.core_features.map((f, i) => <li key={i}>{f}</li>)}
                </ul>
              </div>
              <div>
                <p className="text-text-muted mb-1">Differentiators</p>
                <ul className="list-disc list-inside space-y-0.5">
                  {diagnosis.build_brief.differentiators.map((d, i) => <li key={i}>{d}</li>)}
                </ul>
              </div>
              <div>
                <p className="text-text-muted mb-1">Skip Features</p>
                <ul className="list-disc list-inside space-y-0.5 text-text-faint">
                  {diagnosis.build_brief.skip_features.map((s, i) => <li key={i}>{s}</li>)}
                </ul>
              </div>
              <div>
                <p className="text-text-muted mb-1">Pricing</p>
                <p>{diagnosis.build_brief.pricing_recommendation}</p>
                <p className="text-text-muted mt-2 mb-1">ASO Keywords</p>
                <div className="flex flex-wrap gap-1">
                  {diagnosis.build_brief.aso_keywords.map((k, i) => (
                    <span key={i} className="bg-blue-100 dark:bg-blue-800/40 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded text-xs">{k}</span>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </section>
      )}

      {/* Complaint Clusters */}
      {diagnosis.complaint_clusters && diagnosis.complaint_clusters.length > 0 && (
        <section>
          <h3 className="font-semibold mb-2">Complaint Clusters</h3>
          <div className="space-y-2">
            {diagnosis.complaint_clusters.map((c, i) => (
              <div key={i} className="bg-surface border border-border rounded-lg p-3">
                <div className="flex items-center gap-2 mb-1">
                  <span className="font-medium">{c.label}</span>
                  <span className="text-xs text-text-muted">({c.count} mentions)</span>
                  <span className={`text-xs px-1.5 py-0.5 rounded ${
                    c.severity === 'high' ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400' :
                    c.severity === 'medium' ? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400' :
                    'bg-surface-hover text-text-muted'
                  }`}>{c.severity}</span>
                  <span className="text-xs text-text-faint">requires: {c.requires}</span>
                </div>
                {c.representative_quotes.length > 0 && (
                  <div className="text-sm text-text-muted space-y-1 mt-1">
                    {c.representative_quotes.slice(0, 2).map((q, qi) => (
                      <p key={qi} className="italic text-xs">&ldquo;{q}&rdquo;</p>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Positive Clusters */}
      {diagnosis.positive_clusters && diagnosis.positive_clusters.length > 0 && (
        <section>
          <h3 className="font-semibold mb-2">What Users Love</h3>
          <div className="space-y-2">
            {diagnosis.positive_clusters.map((c, i) => (
              <div key={i} className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-3">
                <span className="font-medium">{c.label}</span>
                <span className="text-xs text-text-muted ml-2">({c.count} mentions)</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Monetisation Complaints */}
      {diagnosis.monetisation_complaints && (
        <section>
          <h3 className="font-semibold mb-2">Monetisation Complaints</h3>
          <div className="grid grid-cols-2 gap-2 text-sm">
            {Object.entries(diagnosis.monetisation_complaints).map(([key, val]) => (
              <div key={key} className="flex items-center gap-2">
                <span className={`w-2 h-2 rounded-full ${val === true ? 'bg-red-400' : val === false ? 'bg-green-400' : 'bg-gray-300'}`} />
                <span className="text-text-muted">{key.replace(/_/g, ' ')}</span>
                {typeof val === 'string' && <span className="font-medium">{val}</span>}
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Recency Analysis */}
      {diagnosis.recency_analysis && (
        <section>
          <h3 className="font-semibold mb-2">Recency Analysis</h3>
          <div className="text-sm space-y-1">
            <p>
              <span className="text-text-muted">Recent negative spike:</span>{' '}
              <span className={diagnosis.recency_analysis.recent_negative_spike ? 'text-red-600 dark:text-red-400 font-medium' : 'text-green-600 dark:text-green-400'}>
                {diagnosis.recency_analysis.recent_negative_spike ? 'Yes' : 'No'}
              </span>
            </p>
            {diagnosis.recency_analysis.recent_complaints_summary && (
              <p className="text-text-muted">{diagnosis.recency_analysis.recent_complaints_summary}</p>
            )}
          </div>
        </section>
      )}
    </div>
  );
}
