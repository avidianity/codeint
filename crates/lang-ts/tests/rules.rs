use std::collections::BTreeSet;

use codeint_lang_ts::{TypeScriptAndJavaScript, flagged_line_rules};
use codeint_model::{LanguagePlugin, RepoPath};

struct RuleFixtures {
    rule: &'static str,
    flagged: &'static [Flagged],
    passing: &'static [&'static str],
}

struct Flagged {
    text: &'static str,
    lines: &'static [u32],
}

const FIXTURES: &[RuleFixtures] = &[
    RuleFixtures {
        rule: "else-after-return",
        flagged: &[
            Flagged {
                text: "export function sign(value: number): string {
  if (value < 0) {
    return 'negative';
  } else {
    return 'positive';
  }
}
",
                lines: &[4],
            },
            Flagged {
                text: "export function label(done: boolean) {
  if (done) return 'done';
  else return 'open';
}
",
                lines: &[3],
            },
            Flagged {
                text: "export function size(count: number): string {
  if (count === 0) {
    return 'none';
  } else if (count === 1) {
    return 'one';
  } else {
    return 'many';
  }
}
",
                lines: &[6],
            },
            Flagged {
                text: "export function pick(first: boolean, second: boolean) {
  if (first) {
    if (second) {
      return 1;
    } else {
      return 2;
    }
  } else {
    return 3;
  }
}
",
                lines: &[5, 8],
            },
            Flagged {
                text: "export class Gate {
  open(key: string) {
    if (!key) {
      return false;
    } else {
      this.unlock(key);
    }
    return true;
  }
  unlock(key: string) {
    console.log(key);
  }
}
",
                lines: &[5],
            },
            Flagged {
                text: "export const parity = (value) => {
  switch (value % 2) {
    case 0:
      if (value === 0) {
        console.log('zero');
        return 'zero';
      } else {
        var even = 'even';
        return even;
      }
    default:
      return 'odd';
  }
};
",
                lines: &[7],
            },
            Flagged {
                text: "export function decide(ready) {
  if (ready) {
    return 'go';
  } else
  // or else wait
  {
    return 'wait';
  }
}
",
                lines: &[4],
            },
            Flagged {
                text: "export function finish(done) {\r\n  if (done) {\r\n    return 1;\r\n  } else {\r\n    return 0;\r\n  }\r\n}\r\n",
                lines: &[4],
            },
        ],
        passing: &[
            "export function sign(value: number): string {
  if (value < 0) {
    return 'negative';
  }
  return 'positive';
}
",
            "export function log(value: number): number {
  if (value < 0) {
    console.log('negative');
  } else {
    return 0;
  }
  return value;
}
",
            "export function size(count: number): string {
  if (count === 0) {
    return 'none';
  } else if (count === 1) {
    return 'one';
  }
  return 'many';
}
",
            "export function size(count: number): string {
  if (count === 0) {
    return 'none';
  } else if (count === 1) {
    console.log('one');
  } else {
    return 'many';
  }
  return 'one';
}
",
            "export function first(items: number[]) {
  if (items.length > 0) {
    for (const item of items) {
      return item;
    }
  } else {
    console.log('empty');
  }
  return undefined;
}
",
            "export function maybe(first: boolean, second: boolean) {
  if (first) {
    if (second) {
      return 1;
    }
  } else {
    return 2;
  }
  return 3;
}
",
            "const total = 1;
export function shadow(empty: boolean) {
  if (empty) {
    return 0;
  } else {
    const total = 2;
    console.log(total);
  }
  return total;
}
",
            "export function scan(items: number[]) {
  for (const item of items)
    if (item > 0) return item;
    else console.log(item);
  return 0;
}
",
        ],
    },
    RuleFixtures {
        rule: "catch-only-rethrows",
        flagged: &[
            Flagged {
                text: "export function load(path: string) {
  try {
    return read(path);
  } catch (error) {
    throw error;
  }
}
",
                lines: &[4, 5, 6],
            },
            Flagged {
                text: "export async function fetchUser(id: string) {
  try { return await get(id); } catch (e) { throw e; }
}
",
                lines: &[2],
            },
            Flagged {
                text: "export class Store {
  save(record: object) {
    try {
      this.write(record);
    }
    catch (failure: unknown) {
      throw (failure);
    }
  }
  write(record: object) {
    console.log(record);
  }
}
",
                lines: &[6, 7, 8],
            },
            Flagged {
                text: "export const parse = (text) => {
  try {
    return JSON.parse(text);
  } catch (error) {
    // let the caller decide
    throw error;
  }
};
",
                lines: &[4, 6, 7],
            },
            Flagged {
                text: "export function outer() {
  try {
    try {
      inner();
    } catch (first) {
      throw first;
    }
  } catch (second) {
    throw second;
  }
}
",
                lines: &[5, 6, 7, 8, 9, 10],
            },
        ],
        passing: &[
            "export function load(path: string) {
  try {
    return read(path);
  } catch (error) {
    throw new Error(`cannot load ${path}`, { cause: error });
  }
}
",
            "export function load(path: string) {
  try {
    return read(path);
  } catch (error) {
    console.error(error);
    throw error;
  }
}
",
            "export function load(path: string) {
  try {
    return read(path);
  } catch (error) {
    throw error;
  } finally {
    close(path);
  }
}
",
            "export function load(path: string, fallback: Error) {
  try {
    return read(path);
  } catch (error) {
    throw fallback;
  }
}
",
            "const failure = new Error('unavailable');
export function load(path: string) {
  try {
    return read(path);
  } catch {
    throw failure;
  }
}
",
            "export function load(path: string) {
  try {
    return read(path);
  } catch ({ message }) {
    throw message;
  }
}
",
            "export function load(path: string) {
  try {
    return read(path);
  } catch (error) {
    throw error.cause;
  }
}
",
        ],
    },
];

fn flagged_lines(rule: &str, text: &str) -> Vec<u32> {
    let path = RepoPath::new("src/fixture.ts").expect("a valid path");
    let model = TypeScriptAndJavaScript
        .lower(&path, text)
        .unwrap_or_else(|error| panic!("the fixture did not lower: {error}\n{text}"));
    model
        .flagged_lines
        .into_iter()
        .filter(|flagged| flagged.rule == rule)
        .map(|flagged| flagged.line)
        .collect()
}

#[test]
fn every_flagged_line_rule_ships_flagged_and_passing_fixtures() {
    let declared: BTreeSet<&str> = flagged_line_rules().collect();
    let with_fixtures: BTreeSet<&str> = FIXTURES
        .iter()
        .filter(|fixtures| !fixtures.flagged.is_empty() && !fixtures.passing.is_empty())
        .map(|fixtures| fixtures.rule)
        .collect();

    assert!(!declared.is_empty());
    assert_eq!(declared, with_fixtures);
}

#[test]
fn each_flagged_fixture_flags_exactly_its_expected_lines() {
    for fixtures in FIXTURES {
        for flagged in fixtures.flagged {
            assert_eq!(
                flagged_lines(fixtures.rule, flagged.text),
                flagged.lines,
                "{}:\n{}",
                fixtures.rule,
                flagged.text
            );
        }
    }
}

#[test]
fn each_passing_fixture_has_no_line_flagged_by_its_rule() {
    for fixtures in FIXTURES {
        for text in fixtures.passing {
            assert_eq!(
                flagged_lines(fixtures.rule, text),
                [0_u32; 0],
                "{}:\n{text}",
                fixtures.rule
            );
        }
    }
}
