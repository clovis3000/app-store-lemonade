---
Document Context:
  Created: 2026-02-14
  Source: Empirical testing while building lemonade (then called arb)
  Status: REFERENCE
  Purpose: Living guide to Apple API endpoints, their limits, and discovery strategies
---

# Apple App Store API Guide

Empirical observations from hitting live endpoints. This is a living document — update as we discover new behaviour or strategies.

---

## Endpoints

### 1. RSS Top Charts

```
GET https://rss.marketingtools.apple.com/api/v2/{country}/apps/{chart}/{limit}/apps.json
```

| Parameter | Values |
|-----------|--------|
| country | us, au, gb, ca, nz (any App Store country code) |
| chart | top-free, top-paid |
| limit | 1-100 |

**Hard limit is 100.** Anything above returns HTTP 500. Not paginated — `page` and `offset` query params are silently ignored and return the same 100 results.

**Rate limits:** Generous. 1s delay is plenty.

**Returns:** Minimal data per app — name, id, artist, artwork URL, genres (with genre ID + name), App Store URL. No ratings, no review counts, no prices.

**Genre filtering:** The API returns top apps across all categories. Filter client-side using the `genres` array. Alternatively, the feed generator UI at `https://rss.marketingtools.apple.com/` may expose genre-specific feeds — worth exploring.

**Response shape:**
```json
{
  "feed": {
    "title": "Top Free Apps",
    "results": [
      {
        "artistName": "Developer Name",
        "id": "123456789",
        "name": "App Name",
        "artworkUrl100": "https://...",
        "genres": [{"genreId": "6002", "name": "Utilities"}],
        "url": "https://apps.apple.com/..."
      }
    ]
  }
}
```

**What it's good for:** Catching popular mainstream apps. Many top-chart apps are big-tech or Games though, so the signal-to-noise for arbitrage is low.

**What it's not good for:** Niche discovery. 100 apps per chart across all categories means most target categories get 0-5 entries.

---

### 2. iTunes Lookup (Metadata Enrichment)

```
GET https://itunes.apple.com/lookup?id={app_id}&country={country}
```

**Bulk lookup (up to 200 IDs per request):**
```
GET https://itunes.apple.com/lookup?id={id1},{id2},{id3},...&country=us
```

**Rate limits:** ~20 req/min. 3s delay works reliably. Bulk lookup counts as 1 request.

**Returns full metadata:** ratings, review counts, price, description, version, release dates, file size, genre, content rating, and more.

**Key fields:**
```json
{
  "resultCount": 1,
  "results": [{
    "trackId": 123456789,
    "trackName": "App Name",
    "sellerName": "Developer",
    "primaryGenreName": "Utilities",
    "averageUserRating": 3.2,
    "userRatingCount": 15420,
    "price": 0.00,
    "formattedPrice": "Free",
    "currentVersionReleaseDate": "2024-06-15T00:00:00Z",
    "releaseDate": "2019-03-01T00:00:00Z",
    "version": "2.1.3",
    "description": "Full app description...",
    "fileSizeBytes": "52428800",
    "trackViewUrl": "https://apps.apple.com/...",
    "averageUserRatingForCurrentVersion": 2.8,
    "isGameCenterEnabled": false,
    "trackContentRating": "4+"
  }]
}
```

**Note:** `averageUserRating` is lifetime. `averageUserRatingForCurrentVersion` is useful — if it's lower than the lifetime average, the app is getting worse.

**What it's good for:** Enriching apps discovered via RSS (which lacks ratings/pricing data). Batch 200 at a time.

**What it's NOT needed for:** Apps discovered via Search — search results already include all these fields.

---

### 3. iTunes Search (Keyword Discovery)

```
GET https://itunes.apple.com/search?term={keyword}&entity=software&country={country}&limit=200
```

**Max 200 results per query.** Rate limit: 3s delay.

**Returns the same fields as Lookup.** This is the key insight — search results come pre-enriched. No need for a separate Lookup call after search.

**Response uses the same shape as Lookup:** `resultCount` + `results[]` with identical fields.

**What it's good for:** This is the primary discovery engine. 56 keywords across target categories yielded 8,356 unique apps in a single country. Niche keywords ("pomodoro", "bmi", "fasting") surface apps that never appear in top charts.

---

### 4. iTunes Review RSS (Review Scraping)

```
GET https://itunes.apple.com/{country}/rss/customerreviews/page={page}/id={app_id}/sortby=mostrecent/json
```

| Parameter | Values |
|-----------|--------|
| country | us, au, gb, ca, nz |
| page | 1-10 |
| sortby | mostrecent, mosthelpful |

~50 reviews per page, 10 pages max = **~500 reviews per country**. Across 5 countries = ~2,500 max per app. No auth needed.

**Gotcha: the `content` field is a single object, not an array.**
```json
{
  "content": {
    "label": "The review text...",
    "attributes": {"type": "text"}
  }
}
```

**Entry structure:**
```json
{
  "feed": {
    "entry": [
      {
        "id": {"label": "13736295586"},
        "title": {"label": "Review title"},
        "content": {"label": "Review body text..."},
        "author": {"name": {"label": "Username"}},
        "im:rating": {"label": "2"},
        "im:version": {"label": "1.8"},
        "updated": {"label": "2026-02-10T19:52:46-07:00"}
      }
    ]
  }
}
```

**Observed yields:** Countdown App (35K lifetime reviews) returned 1,779 via RSS across 5 countries. Distribution: US 500, GB 500, CA 500, AU 244, NZ 35. The 500 cap per country is real.

**When pages run out:** The endpoint returns valid JSON with no `entry` key (or an empty feed). It doesn't 404 — you just stop getting entries. So an HTTP error on pages 1-10 is a real failure (throttling, outage), not the end of the feed. Page 11 and beyond return 400. (Re-checked 2026-10-22.)

**Gotcha: `entry` is a bare object when a page holds exactly one review**, not a one-element array. Deserialize it as one-or-many.

---

## Discovery Strategies

### Current (v1)

1. **RSS top charts** — 100 free + 100 paid per country. Good for mainstream, noisy for arbitrage.
2. **Keyword search** — 56 niche keywords, 200 results each. This is where the volume comes from.

### Ideas to Explore

- **Expand keyword list.** Current list is heavily health/fitness/utility weighted. Could add: "invoice", "receipt", "timesheet", "inventory", "checklist", "kanban", "diagram", "pdf", "signature", "voice recorder", "sound meter", "altimeter", "speedometer", "astronomy", "tide", "fishing", "hiking", "cycling", "yoga", "stretching", "posture", "eye test", "hearing test", "blood pressure".
- **Two-word compound keywords.** "habit tracker", "water reminder", "sleep sounds", "focus timer", "meal prep", "net worth", "debt payoff", "savings goal", "bill splitter". The search API handles multi-word terms.
- **Category-specific RSS feeds.** The feed generator at `https://rss.marketingtools.apple.com/` may support genre filters we haven't tried. Worth inspecting.
- **Autocomplete mining.** `https://search.itunes.apple.com/WebObjects/MZSearchHints.woa/wa/hints?term={prefix}` may return autocomplete suggestions — could be used to discover high-volume search terms.
- **Related apps.** After identifying a target app, search for its ASO keywords to find the cluster of competitors. The competitive scan already does this during diagnosis.
- **Multi-country search.** Currently searching US only. AU/GB/CA/NZ may surface region-specific apps not in the US store.
- **Temporal re-scraping.** Re-run keyword searches periodically to catch newly neglected apps whose last_updated ages past the threshold.

---

## Rate Limit Summary

| Endpoint | Observed Safe Delay | Notes |
|----------|-------------------|-------|
| RSS Top Charts | 1s | Very generous |
| iTunes Lookup | 3s | ~20 req/min. Bulk (200 IDs) counts as 1 request |
| iTunes Search | 3s | Same rate as Lookup |
| Review RSS | 3s per page | No auth, no known hard limits. Backs off gracefully |
