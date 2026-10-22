# Query recipes

For the agent driving lemonade: reading the review index with `sqlite3 data/db/lemonade.db`.

The index has three layers:

- **Labels** in `review_labels`: one row per review per profile, with the labels as JSON in `labels_json`. The fields are whatever that profile's `classify.schema.json` defines. Read the schema with `lemonade profile show <name>` rather than assuming the general fields below.
- **Embeddings** in `review_embeddings`, a sqlite-vec table shared by all profiles. Search them with `lemonade query`, which embeds the query text via OpenAI.
- **Keywords** in `reviews_fts`, an FTS5 table with BM25 ranking.

The recipes below use the general profile's fields. Swap the profile name, and the field names if the profile's taxonomy differs.

## Unpacking labels

Each recipe starts from this view of one profile's labels:

```sql
WITH l AS (
  SELECT r.*, a.name AS app_name, x.labels_json AS labels
  FROM review_labels x
  JOIN reviews r ON r.review_id = x.review_id
  JOIN apps a ON a.app_id = r.app_id
  WHERE x.profile = 'general'
)
SELECT ... FROM l ...;
```

Scalar fields: `json_extract(labels, '$.sentiment')`. Array fields: `JOIN json_each(labels, '$.themes') j`, then `j.value`.

## Churn: who's leaving and why

```sql
SELECT app_name, rating, title, body, json_extract(labels, '$.emotional_tone') AS tone
FROM l
WHERE json_extract(labels, '$.user_context') = 'churning'
ORDER BY json_extract(labels, '$.intensity') DESC
LIMIT 30;
```

## Where users go

```sql
SELECT app_name AS from_app, json_extract(labels, '$.competitor_mentioned') AS to_app, COUNT(*) AS n
FROM l
WHERE json_extract(labels, '$.user_context') IN ('switching_out', 'churning')
  AND json_extract(labels, '$.competitor_mentioned') IS NOT NULL
GROUP BY 1, 2 ORDER BY n DESC;
```

## Paywall friction by app

```sql
SELECT app_name, COUNT(*) AS complaints, ROUND(AVG(json_extract(labels, '$.intensity')), 1) AS intensity
FROM l JOIN json_each(labels, '$.themes') j ON j.value = 'paywall'
WHERE json_extract(labels, '$.sentiment') = 'negative'
GROUP BY app_name ORDER BY complaints DESC;
```

## What will people pay for

```sql
SELECT app_name, rating, title, body
FROM l
WHERE json_extract(labels, '$.mentions_premium') = 1
  AND json_extract(labels, '$.premium_sentiment') = 'positive'
ORDER BY rating DESC LIMIT 30;
```

## Most-requested and most-complained-about features

```sql
SELECT j.value AS feature, json_extract(labels, '$.review_intent') AS intent, COUNT(*) AS n
FROM l JOIN json_each(labels, '$.features') j
WHERE json_extract(labels, '$.review_intent') IN ('complaint', 'feature_request')
GROUP BY 1, 2 ORDER BY n DESC LIMIT 40;
```

## Underserved segments

```sql
SELECT json_extract(labels, '$.user_segment') AS segment, COUNT(*) AS n,
       SUM(json_extract(labels, '$.sentiment') = 'negative') AS negative
FROM l
WHERE json_extract(labels, '$.user_segment') IS NOT NULL
GROUP BY 1 HAVING n >= 3 ORDER BY negative DESC;
```

## Personal stories with strong feeling

```sql
SELECT app_name, rating, title, body, json_extract(labels, '$.emotional_tone') AS tone
FROM l
WHERE json_extract(labels, '$.contains_personal_story') = 1
  AND json_extract(labels, '$.intensity') >= 4
ORDER BY date DESC LIMIT 20;
```

## Apps an update broke

```sql
SELECT app_name, COUNT(*) AS complaints, GROUP_CONCAT(DISTINCT version) AS versions
FROM l JOIN json_each(labels, '$.themes') j ON j.value = 'update_regression'
GROUP BY app_name ORDER BY complaints DESC;
```

## Sentiment over time

```sql
SELECT app_name, SUBSTR(date, 1, 7) AS month, COUNT(*) AS reviews,
       SUM(json_extract(labels, '$.sentiment') = 'negative') AS negative,
       ROUND(AVG(rating), 1) AS avg_rating
FROM l WHERE date IS NOT NULL
GROUP BY 1, 2 HAVING reviews >= 5 ORDER BY 1, 2;
```

## Keyword search

```sql
-- Terms are ANDed; use OR, or quotes for a phrase.
SELECT r.app_id, r.rating, r.title, r.body, f.rank
FROM reviews_fts f JOIN reviews r ON r.review_id = f.review_id
WHERE f.reviews_fts MATCH '"lost my data" OR "lost everything"'
ORDER BY f.rank LIMIT 20;
```

## Semantic search

```bash
lemonade query "app that used to be great but declined"
lemonade query "lost years of data after an update" --app-id 1234567890 --limit 10
lemonade query "switching because of ads" --profile habit-trackers --json --limit 30
```

Semantic search finds reviews by meaning, not by keyword. Use it for vague concepts and experiences, and FTS for exact terms. `--profile` attaches that profile's labels to each result.

## Research results

```sql
SELECT id, name, profile, created_at FROM syntheses ORDER BY id DESC;
SELECT result_json FROM syntheses WHERE id = 1;                 -- pipe to jq
SELECT id, app_id, prompt_name, profile, parent_id FROM analyses ORDER BY id DESC LIMIT 20;
```
