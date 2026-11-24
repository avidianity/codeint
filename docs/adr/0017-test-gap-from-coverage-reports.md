---
status: accepted
---

# codeint measures Test gap from Coverage reports and rejects CRAP

The Owner decided this on 2026-11-24 after asking for CRAP.

## Context

CRAP (Change Risk Anti-Patterns) gives each function one number: cyclomatic complexity squared, times the untested share cubed, plus the complexity.
A function with a CRAP greater than 30 counts as risky.
With no coverage facts, that means a cyclomatic complexity of 6 or more, which is Erosion with a lower limit.
So the complexity half of CRAP repeats what Erosion and Function mass already weigh.
The half that is new is test coverage, and nothing in codeint read test coverage before this decision.
codeint never runs the code of the project that it measures.
Checked on 2026-11-24: Bun 1.4, Node 26, `dart test` with package:test 1.33, and `flutter test` write LCOV as their only coverage file format.
PHPUnit 13.4 and Pest 5 write Cobertura XML and cannot write LCOV.
cargo-llvm-cov 0.9.2, Vitest 5.1, Jest 30.5, and coverage.py 7.17 write both.

## Decision

**codeint reports a Test gap Metric, the share of total Function mass that tests do not execute, and reads test execution from Coverage reports that the user names.**
codeint never runs tests and never guesses where a Coverage report is.
The user names a list of Coverage reports, and a line counts as executed when any report executed it.
codeint reads LCOV and Cobertura XML, and tells the two apart by content.
The untested share of a function is its lines that a report lists with a count of 0, divided by all its lines that a report lists.
Branch facts are ignored.
Test gap covers the Production Source set only.
codeint adds Coverage report facts to the Code model before any Metric runs, so a Metric plugin still computes from the Code model alone.

## Considered options

- **CRAP with its limit of 30, as a share of Function mass or of functions.**
  Rejected: it squares the complexity that Erosion already weighs, so the two Metrics move together.
- **The untested share of eroded functions only.**
  Rejected: it has no value when no function is eroded, and it depends on Erosion by definition.
- **Guess common report paths.**
  Rejected: a report left on disk from an older test run gives a wrong value with no sign of error.
- **codeint runs the tests.**
  Rejected: codeint never runs the code that it measures.
- **Test gap reads the Coverage report beside the Code model.**
  Rejected: a third-party Metric plugin could then never use coverage.
- **Count branches as well as lines.**
  Rejected for now: branch facts are optional in both formats and differ by tool.

## Consequences

- Test gap moves only when test execution moves, so it never repeats Erosion.
- Test gap is absent, never 0 or 1, when no Coverage report is named, or when the reports hold no file of a language.
- A function with no entry in a report that covers its language counts as fully untested, because many test tools leave out files that the tests never loaded.
- codeint matches a report path to a repository file by the longest matching ending, and names every path that matches no file.
- codeint stops with an error when a report cannot be parsed, or when it lists a line beyond the end of its file.
  It cannot detect any other stale report.
- Test gap has a Budget and no Regression check, because no Coverage report exists for the merge-base of a base Git ref.
- Test gap has no Reference range, because measuring a panel means running the tests of every panel repository.
- The Code model gains coverage facts per function, which needs a new version of the Plugin interface.
