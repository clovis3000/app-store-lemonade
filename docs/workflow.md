# Workflow: general research to tailored research

For the agent driving lemonade. The human says what market they're curious about; you run the commands, read the database, and report back in plain language. Exact flags are in `lemonade <command> --help`; this doc covers the order of work and the judgement calls.

## 1. Fill the database

```bash
lemonade harvest --search-only        # keyword search, using `keywords` in lemonade.toml
lemonade harvest                      # plus top charts and enrichment
lemonade score
lemonade list shortlist --limit 40
```

To explore a market not in the keyword list, add its terms to `keywords` in `lemonade.toml` and run `harvest --search-only` again. `research` and `index` only see apps already in the database.

The opportunity score is a pre-filter, not a verdict. It ranks demand × dissatisfaction × neglect, and every one of those signals has a failure mode that looks like opportunity. Use it to decide which reviews to read, never to decide what to build.

## 2. General research

```bash
lemonade research --search "habit tracker,streaks" --top 8 --plan    # always plan first: cost, time, scraping
lemonade research --search "habit tracker,streaks" --top 8 --name "Habit trackers, first pass"
```

Check the plan's app list before running. Search ranks name matches first, but when few apps carry the term in their name, description matches fill the list and pull in strays. Pin the ones you mean by researching `--app-id` individually, or by setting `ids` in a profile (step 3).

Each app gets an analysis: praised, hated, wishlist, what users pay for and why they cancel, switching signals, and underserved segments. With more than one app, a cross-app synthesis follows: universal complaints, feature gaps, underserved opportunities, and recommended positioning. Read them:

```bash
lemonade research --list
sqlite3 data/db/lemonade.db "SELECT result_json FROM syntheses WHERE id = <id>" | jq .
```

Dig into anything surprising with a follow-up. It reads the parent analysis plus the reviews it cited:

```bash
lemonade research --app-id <id> --parent <analysis id> --followup "Why do long-term users leave after the redesign?"
```

## 3. Tailor a profile

General prompts find general things. Once the first pass shows what this market cares about, give it a profile:

```bash
lemonade profile new habit-trackers --description "Habit and streak apps" --search "habit tracker,streaks" --top 8
```

Then edit the copies in `profiles/habit-trackers/`:

- **`classify.schema.json`**: replace free-text fields with this market's enums. If the first pass kept seeing "streak freeze", "widgets" and "Apple Watch", make those `features` values. If certain kinds of user kept appearing, give `user_segment` an enum. Free text splits one complaint into five spellings; enums make counts mean something.
- **`classify.md`**: the rules for each field, including what *not* to infer.
- **`competitive_research.tera`**: point the analysis at what matters here, such as segments to watch for or features to judge.

Delete any copied file you didn't change, so it keeps following the general version. Profiles are private by default (`profiles/` is gitignored), because they encode the human's product thinking.

## 4. Tailored research and indexing

```bash
lemonade index --profile habit-trackers --calibrate     # ~200 stratified reviews; read the label distributions
# adjust the taxonomy until 'other' is rare and no value swallows half the reviews, then:
lemonade index --profile habit-trackers
lemonade research --profile habit-trackers --name "Habit trackers, tailored"
```

Calibration writes under `habit-trackers:calibration` and never touches real labels. `index` labels every review of the profile's apps, embeds them for `query`, and is safe to rerun: it only does what's missing.

Now the database answers questions directly. `docs/query-recipes.md` has SQL for churn, where users go, paywall friction, what people pay for, feature gaps and underserved segments:

```bash
lemonade query "gave up because it made me feel guilty" --profile habit-trackers --limit 20
```

## 5. Report

Lead with what to build and for whom, then the evidence: counts, plus quotes with review IDs. Say what the data can't show. Reviews skew toward the angry and the delighted, the RSS feed caps at 500 per country, and a 2.5-star average from 2019 says nothing about today.
