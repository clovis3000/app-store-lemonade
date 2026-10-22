# lemonade

Find apps people are sour about, and make something better.

lemonade reads the App Store for you. It finds apps with proven demand and unhappy users, reads their reviews with Claude, and tells you what people love, what they hate, what they'd pay for, and who nobody is serving. You don't operate it yourself. Your coding agent does, and you ask it questions:

> "Which habit trackers have lots of users and bad reviews, and what would people pay for that none of them do?"

It's a single Rust binary with a local SQLite database. It needs no accounts and no server, and it calls nothing except Apple's public endpoints and the model APIs.

## How it works

1. **Harvest**: pull apps from App Store top charts and keyword search, with ratings, prices and update dates.
2. **Score**: rank apps by demand × dissatisfaction × neglect. This is a rough filter for which reviews are worth reading.
3. **Research**: scrape up to 500 recent reviews per country per app. Claude Haiku reads them in batches and writes up what users love, hate, wish for and pay for, and who is underserved. Claude Opus then synthesises the market across apps.
4. **Tailor**: copy the general prompts into a *profile* for your market, so the analysis looks for what matters there.
5. **Index**: label every review with your profile's taxonomy and embed it, so your agent can query the market directly ("show me churn by app", "reviews like 'it made me feel guilty'").

## Quick start

```bash
git clone https://github.com/clovis3000/app-store-lemonade && cd app-store-lemonade
cargo build --release
export ANTHROPIC_API_KEY=...        # research and diagnose
export OPENAI_API_KEY=...           # index and query (classification and embeddings)

./target/release/lemonade harvest --search-only
./target/release/lemonade research --search "habit tracker" --top 6 --plan
```

Then hand it to your agent. If you use Claude Code, open it in this folder and ask a question about a market: `CLAUDE.md` and [`docs/workflow.md`](docs/workflow.md) tell it how to drive lemonade. Any agent that can run shell commands and read SQLite can drive it the same way.

`--plan` shows the cost before anything runs. A typical eight-app market costs well under a dollar.

## Profiles

`prompts/` holds the general research prompts and review taxonomy. A profile in `profiles/<name>/` overrides any of them for one market, such as health trackers, budgeting apps or kids' learning games. Whatever it doesn't override follows the general version. The usual loop:

general research → spot what this market cares about → `lemonade profile new` → tailor the copies → tailored research and indexing

Git ignores `profiles/`, along with your database. They are your product research, and they stay yours. See [`profiles/README.md`](profiles/README.md).

## Commands

| Command | What it does |
|---|---|
| `harvest` | Discover apps from top charts and keyword search |
| `score` | Rank apps by opportunity |
| `research` | Per-app analysis and cross-app synthesis; `--followup` digs into one |
| `profile` | Create and inspect research profiles |
| `index` | Label reviews with a profile's taxonomy and embed them |
| `query` | Semantic search over reviews |
| `diagnose` | A build brief for a top-scored app, with a competitor scan |
| `list`, `card` | Read results in the terminal |
| `serve` | Local web GUI for browsing (`npm run build --prefix web` first) |

Every command takes `--help`. [`docs/query-recipes.md`](docs/query-recipes.md) has SQL for reading results, and [`docs/API-GUIDE.md`](docs/API-GUIDE.md) records how Apple's endpoints actually behave.

## Configuration

`lemonade.toml` sets countries, harvest keywords, scoring thresholds, rate limits and models. lemonade finds it by searching upward from the current directory, and paths in it resolve relative to the file, so commands work from anywhere inside the checkout.

## Caveats

- Apple's review feed returns at most 500 reviews per country per app, newest first. Reviews skew toward people with strong feelings.
- The opportunity score multiplies three noisy signals. Treat it as a filter, not a verdict.
- Scraping is rate-limited to stay polite to Apple's public endpoints. A full eight-app run takes around 15 minutes, mostly waiting.

## License

MIT. See [LICENSE](LICENSE).
