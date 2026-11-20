---
status: proposed
---

# TypeScript and JavaScript Reference ranges come from a panel of 20 public repositories

Ticket #74 of Spec #10 proposes this record on 2026-11-21, for the Owner's approval.
The published Reference ranges cover Python only, as ADR 0003 records, so codeint measures its own for TypeScript and JavaScript.

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
A panel picked after looking at results can be bent toward any range, so the selection rule below was written and committed before any repository was measured.

### Selection rule

The rule was written on 2026-11-21, before any measurement.

1. Run two GitHub repository searches on 2026-11-21, each sorted by stars in descending order:
   - `language:TypeScript created:<2022-01-01 fork:false archived:false`
   - `language:JavaScript created:<2022-01-01 fork:false archived:false`
2. Walk each result list in order, and take the first 10 repositories that pass every exclusion below.
   The panel is those 20 repositories: 10 from each search.
3. Pin each repository at the last commit of its default branch dated before 2023-01-01, by the committer date.
   Code written before 2023 predates the wide use of coding agents, so it stands for human-written code.
4. Record each repository's stars band from its star count on the query date: 10k to 25k, 25k to 50k, 50k to 100k, or over 100k.

A repository is excluded, and the next one in its list takes its place, when any of these is true:

- It is a fork, a mirror, or archived.
- Its description or topics say it is a list, a roadmap, a curriculum, a course, a book, a tutorial, a set of interview questions, a cheat sheet, or a collection of snippets or examples.
- At the pinned commit, codeint counts fewer than 5,000 source lines in the Production Source set.
- At the pinned commit, the Generated and Vendored Source sets together hold more source lines than the Production Source set.
- codeint exits with an error on it; the record lists each such repository and the error.

No repository is removed after measurement for any other reason.
An outlier stays in the panel and is named, with its likely cause.

### Method

- Build codeint at the commit that this record names, with `cargo build --release --locked`.
- Use the default rule set: a minimum clone size of 75 tokens and a clone filter of 14 distinct normalized tokens, as ADR 0007 decided.
- Fetch each pinned commit with depth 1 into a scratch folder outside the codeint repository.
- Run `codeint check --format json --all` on each, and read each Metric's value in the Production Source set.
- Report, per Metric, the mean, the median, the standard deviation, and the 10th to 90th percentile.
- Record only numbers; no code, fixture, or text from a panel repository enters codeint.
