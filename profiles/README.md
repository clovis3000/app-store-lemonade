# Research profiles

A profile tailors lemonade to one market. Git ignores everything in this folder except this file, because your profiles are your product research.

`prompts/` is the general profile. A tailored profile is a folder here holding a `profile.toml` and any prompt files it overrides. Files it doesn't override fall through to `prompts/`, so improvements there keep reaching it.

```bash
lemonade profile new habit-trackers --description "Habit and streak apps" --search "habit tracker,streaks" --top 8
lemonade profile show habit-trackers
```

`profile new` copies in the three files that differ most between markets:

| File | What tailoring it changes |
|---|---|
| `competitive_research.tera` | What the per-app analysis looks for. For example, which user segments matter in this market |
| `classify.md` | The rules `lemonade index` gives the classifier |
| `classify.schema.json` | The label taxonomy: domain enums for themes, features and segments |

Delete any copied file you leave unchanged, so it keeps tracking the general version. To override any other file in `prompts/`, copy it in here.

`profile.toml`:

```toml
description = "Habit and streak apps"

[apps]
search = ["habit tracker", "streaks"]  # matched against app names and descriptions
ids = ["1234567890"]                    # always included
top = 8                                 # search matches to keep, by rating count
```

Search ranks apps with a term in their name first, then apps that only mention it in their description, which can include strays. Pin the apps you mean with `ids` once you know them.

The research output schema (praised, hated, wishlist, premium, switching, segments) is fixed, because synthesis and the GUI read it. The classifier schema is entirely the profile's own, and labels are stored as JSON per profile in `review_labels`.
