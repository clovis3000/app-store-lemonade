# lemonade

Find apps people are sour about, and make something better: App Store apps with proven demand and unhappy users, read through their reviews to learn what to build instead and for whom.

## The agent is the user

The human rarely types `lemonade`. An agent (Claude Code or similar) runs it, reads the results from SQLite, and answers the human's questions about a market. So:

- **Commands do the work and report what happened.** No interactive prompts, no confirmation gates; `--plan` exists for looking before spending.
- **The database is the interface.** Results land in SQLite and the agent reads them with `sqlite3`. Don't build export formats, MCP servers or special agent APIs; the schema is the API.
- **Throughput over ergonomics.** Batch aggressively, store everything, and let the agent filter.
- **The human's research is theirs.** Profiles and the database encode their product thinking. They stay out of git by default, and nothing specific to one market belongs in the shipped prompts or code.

The order of work and the judgement calls are in `docs/workflow.md`. The command surface is `lemonade --help` (`src/cli.rs`); don't mirror it here.

## The score is a pre-filter

`demand × dissatisfaction × neglect × monetisation` reduces thousands of apps to the hundred worth reading reviews for. Each signal has a failure mode that looks identical to opportunity: neglected can mean a dead market, a low rating can be structural to the category, and a high historical rating count says nothing about demand today. The value is in the reviews, read in context.

## Canonical sources

- Profile mechanics: `profiles/README.md` and `src/profile.rs`. Don't restate the override rules elsewhere.
- Reading results: `docs/query-recipes.md`.
- Apple endpoint behaviour: `docs/API-GUIDE.md`. Record new empirical findings there, with a date.
- Model prices: `price_per_mtok` in `src/diagnose/llm.rs`, the only place any price lives.

## Anti-patterns

### Don't put one market's taxonomy in `prompts/` or in code (2026-10-22)

`index.rs` once hard-coded one niche's classifier: its system prompt, enums, and dedicated columns on `reviews`. Every other market got labels that only made sense for that niche, and the public code gave away whose research it was. A market's taxonomy belongs in its profile; `prompts/` stays general.

### Don't commit `profiles/`, `private/`, `data/` or `CLAUDE.local.md` (2026-10-22)

The repo is public and these hold users' product research. `.gitignore` covers them, but `git mv` and `git add -f` walk straight past it: strategy docs got committed this way on 2026-10-22, and only a re-rooted history kept them off GitHub.

### Don't slice `&str` by byte index (2026-10-22)

`&s[..n]` panics mid-character. App names and reviews are full of emoji, accents and currency symbols: 28 app names and 157 reviews in the live DB crashed `research --plan` and `query`. Use `text::truncate`.

### Don't treat an HTTP error as the end of Apple's review feed (2026-10-22)

The feed ends with an empty 200. An error on pages 1-10 is throttling or an outage. Logging it as a successful scrape meant `diagnose`, which never re-scrapes, kept that app's country empty for good.

### Don't pair results across a filtered list by position (2026-10-22)

Cross-app synthesis took `apps[i]` for the i-th successful analysis. One failed app shifted every later analysis onto the wrong app's name and rating. Carry the pairing with the result.

### Don't keep a second record of state that has a source of truth (2026-10-22)

`reviews.embedded_at` drifted from the vec0 table: 99 reviews were marked embedded with no vector, so they were invisible to search and never retried. Ask the table that holds the thing.

### Don't let calibration or test paths write to shared state (2026-10-22)

`index --calibrate` once nulled every label and deleted every embedding to get "clean" stats. Calibration now writes under `<profile>:calibration`.

### Don't hand-write LLM output schemas or parse JSON out of free text (2026-10-22)

Output schemas derive from the Rust result structs (`output_schema::<T>()`), and calls use structured outputs. Before that, prose instructions plus fence-stripping produced truncated JSON that was retried identically. Check `stop_reason` (`max_tokens`, `refusal`) before reading content, and read the `text` block, not `content[0]`: thinking models put thinking blocks first.

### Don't fall back to defaults when the config is missing (2026-10-22)

Relative paths plus a silent default config created a fresh empty database wherever a command ran (one was found in `web/data/`). Config is found by searching upward for `lemonade.toml`, and its paths resolve against it.

### Don't store research results in `apps.diagnosis_json`

`diagnose` writes a build brief there (1:1, overwritten). `research` writes to `analyses` and `syntheses` (1:many, chained by `parent_id`). The old `diagnose --prompt competitive` broke this until 2026-10-22.

### Don't add review staleness to `diagnose`, or remove it from `research`

`research` re-scrapes after `review_staleness_days` (14), because competitive research wants current reviews. `diagnose` scrapes once, because it's a one-shot ranking pass. The difference is intentional.

### Don't raise the research batch size past ~25

The research output schema is large. Batches of 50 truncated at the token limit and failed to parse, and thinking models spend part of `max_tokens` on thinking before they write.

### Don't trust Apple RSS top-chart limits above 100

The feed generator suggests 200, but anything over 100 returns HTTP 500 with no explanation, and `page`/`offset` are silently ignored.

### Don't model the review feed's `content` as an array

It's a single object `{label, attributes}`. With `Vec<>`, serde silently produces zero reviews. (`entry` is the reverse case: an array, except on a page holding exactly one review.)

### Don't make Lookup calls for apps found via Search

Search returns the same enriched fields. A Lookup call spends rate-limit budget (~20 req/min) on data you already have.

### Don't make query functions async

The web API wraps `Database` in `Arc<Mutex<_>>` and calls the shared sync queries inside `spawn_blocking`. Making them async would break the CLI path for no gain.

### Don't use flat axum routes with repeated path parameters

`/api/apps/{app_id}` beside `/api/apps/{app_id}/star` conflicts in axum 0.8, and POSTs fall through to the static fallback as a confusing 405. Group sub-resources with `nest`.

### Don't sprinkle `dark:` variants across frontend components

Theming is semantic tokens in `web/src/index.css`, swapped by `.dark` on `<html>`. Use `dark:` only for accent overrides that tokens can't express.

### Don't remove `#![allow(dead_code)]` from `main.rs`

Many fields exist only for serde (de)serialization, and rustc can't see that use.
