use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use codeint_model::Metric;
use codeint_plugin_host::{built_in_language_flagged_line_rules, built_in_language_hazard_rules};
use serde_json::Value;

use crate::common::{init_git_repo, run_codeint_text};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    metric: String,
    path: String,
    line: u64,
    rule: String,
}

struct Plant {
    metric: &'static str,
    rule: &'static str,
    path: &'static str,
    lines: (u64, u64),
}

const fn flagged(rule: &'static str, path: &'static str, lines: (u64, u64)) -> Plant {
    Plant {
        metric: "verbosity",
        rule,
        path,
        lines,
    }
}

const fn hazard(rule: &'static str, line: u64) -> Plant {
    Plant {
        metric: "hazards",
        rule,
        path: "db/statements.sql",
        lines: (line, line),
    }
}

const fn hazard_in(rule: &'static str, path: &'static str, line: u64) -> Plant {
    Plant {
        metric: "hazards",
        rule,
        path,
        lines: (line, line),
    }
}

const fn schema_hazard(rule: &'static str, line: u64) -> Plant {
    Plant {
        metric: "hazards",
        rule,
        path: "db/schema.sql",
        lines: (line, line),
    }
}

struct RuleSuite {
    plugin: &'static str,
    files: &'static [(&'static str, &'static str)],
    plants: &'static [Plant],
}

fn expected_findings(plants: &[Plant]) -> BTreeSet<Finding> {
    plants
        .iter()
        .flat_map(|plant| {
            (plant.lines.0..=plant.lines.1).map(|line| Finding {
                metric: plant.metric.to_owned(),
                path: plant.path.to_owned(),
                line,
                rule: plant.rule.to_owned(),
            })
        })
        .collect()
}

fn reported_line_findings(report: &Value) -> BTreeSet<Finding> {
    let mut found = BTreeSet::new();
    let source_sets = report["source_sets"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for section in source_sets.values() {
        for metric in ["verbosity", "hazards"] {
            for finding in section[metric]["findings"].as_array().into_iter().flatten() {
                found.insert(Finding {
                    metric: metric.to_owned(),
                    path: finding["path"].as_str().unwrap_or_default().to_owned(),
                    line: finding["line"].as_u64().unwrap_or_default(),
                    rule: finding["rule"].as_str().unwrap_or_default().to_owned(),
                });
            }
        }
    }
    found
}

fn assert_full_recall_and_no_false_positives<T: Ord + std::fmt::Debug>(
    label: &str,
    expected: &BTreeSet<T>,
    reported: &BTreeSet<T>,
) {
    let missed: Vec<&T> = expected.difference(reported).collect();
    let false_positives: Vec<&T> = reported.difference(expected).collect();
    let recalled = expected.len() - missed.len();
    assert!(
        missed.is_empty() && false_positives.is_empty(),
        "{label}: recall {recalled}/{}, missed {missed:?}, false positives {false_positives:?}",
        expected.len()
    );
}

fn check_all_json(root: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["check", "--format", "json", "--all"];
    args.extend_from_slice(extra);
    let (stdout, _) = run_codeint_text(root, &args, 0);
    serde_json::from_str(&stdout).expect("valid json report")
}

fn run_rule_suite(suite: &RuleSuite) {
    let label = suite.plugin;
    let repo = init_git_repo(suite.files);
    let report = check_all_json(repo.path(), &[]);
    let expected = expected_findings(suite.plants);
    assert_full_recall_and_no_false_positives(label, &expected, &reported_line_findings(&report));
    let planted_flagged_rules: BTreeSet<&str> = suite
        .plants
        .iter()
        .filter(|plant| plant.metric == "verbosity")
        .map(|plant| plant.rule)
        .collect();
    let registered: BTreeSet<String> = built_in_language_flagged_line_rules()
        .remove(label)
        .unwrap_or_else(|| panic!("{label} is not a built-in Language plugin"))
        .into_iter()
        .collect();
    let planted_flagged_rules: BTreeSet<String> = planted_flagged_rules
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        planted_flagged_rules, registered,
        "{label}: every Flagged line rule has a plant"
    );
}

const TYPESCRIPT_RULES: &str = r#"export function sign(value: number): string {
  if (value < 0) {
    return "negative";
  } else {
    return "positive";
  }
}

export function signClean(value: number): string {
  if (value < 0) {
    return "negative";
  }
  return "positive";
}

export function isAdult(age: number): boolean {
  if (age >= 18) {
    return true;
  } else {
    return false;
  }
}

export function isAdultClean(age: number): boolean {
  return age >= 18;
}

export const ready = (count: number): boolean => count > 3 ? true : false;

export const label = (count: number): string => count > 3 ? "many" : "few";

export function log(message: string): void {
  console.log(message);
  return;
}

export function logClean(message: string): void {
  console.log(message);
}

export function load(read: () => string): string {
  try {
    return read();
  } catch (error) {
    throw error;
  }
}

export function loadClean(read: () => string): string {
  try {
    return read();
  } catch (error) {
    console.error(error);
    throw error;
  }
}

class Base {
  constructor(public name: string) {}
}

export class Child extends Base {
  constructor(...args) {
    super(...args);
  }
}

export class ChildClean extends Base {
  constructor(name: string) {
    super(name.trim());
  }
}
"#;

const TYPESCRIPT_PLANTS: &[Plant] = &[
    flagged("else-after-return", "src/rules.ts", (4, 4)),
    flagged("else-after-return", "src/rules.ts", (19, 19)),
    flagged("if-else-returns-boolean", "src/rules.ts", (17, 21)),
    flagged("conditional-yields-boolean", "src/rules.ts", (28, 28)),
    flagged("trailing-bare-return", "src/rules.ts", (34, 34)),
    flagged("catch-only-rethrows", "src/rules.ts", (44, 46)),
    flagged("constructor-only-forwards", "src/rules.ts", (63, 65)),
];

const RUST_RULES: &str = r#"pub fn sign(value: i32) -> &'static str {
    if value < 0 {
        return "negative";
    } else {
        "positive"
    }
}

pub fn sign_clean(value: i32) -> &'static str {
    if value < 0 { "negative" } else { "positive" }
}

pub fn is_adult(age: u32) -> bool {
    if age >= 18 { true } else { false }
}

pub fn is_adult_clean(age: u32) -> bool {
    age >= 18
}

pub fn double(value: i32) -> i32 {
    return value * 2;
}

pub fn double_clean(value: i32) -> i32 {
    value * 2
}

pub fn parse(input: Result<i32, String>) -> Result<i32, String> {
    let value = match input {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    Ok(value + 1)
}

pub fn parse_clean(input: Result<i32, String>) -> Result<i32, String> {
    let value = input?;
    Ok(value + 1)
}

pub fn invert(flag: bool) -> bool {
    match flag {
        true => false,
        false => true,
    }
}

pub fn invert_clean(flag: bool) -> bool {
    !flag
}

fn triple(value: i32) -> i32 {
    value * 3
}

pub fn triple_all(values: Option<i32>) -> Option<i32> {
    values.map(|value| triple(value))
}

pub fn triple_all_clean(values: Option<i32>) -> Option<i32> {
    values.map(triple)
}
"#;

const RUST_PLANTS: &[Plant] = &[
    flagged("else-after-return", "src/lib.rs", (4, 6)),
    flagged("if-else-returns-boolean", "src/lib.rs", (14, 14)),
    flagged("trailing-return", "src/lib.rs", (22, 22)),
    flagged("match-only-propagates", "src/lib.rs", (30, 33)),
    flagged("match-yields-boolean", "src/lib.rs", (43, 46)),
    flagged("redundant-closure", "src/lib.rs", (58, 58)),
];

const PHP_RULES: &str = r#"<?php

function sign($value)
{
    if ($value < 0) {
        return 'negative';
    } else {
        return 'positive';
    }
}

function signClean($value)
{
    if ($value < 0) {
        return 'negative';
    }
    return 'positive';
}

function isAdult($age)
{
    if ($age >= 18) {
        return true;
    } else {
        return false;
    }
}

function ready($count)
{
    return $count > 3 ? true : false;
}

function label($count)
{
    return $count > 3 ? 'many' : 'few';
}

function load($read)
{
    try {
        return $read();
    } catch (Exception $error) {
        throw $error;
    }
}

function loadClean($read, $log)
{
    try {
        return $read();
    } catch (Exception $error) {
        $log($error);
        throw $error;
    }
}

function notify($send)
{
    $send();
    return;
}

function notifyClean($send, $enabled)
{
    if (!$enabled) {
        return;
    }
    $send();
}

class Child extends Base
{
    public function __construct($name, $age)
    {
        parent::__construct($name, $age);
    }
}

class ChildClean extends Base
{
    public function __construct($name, $age)
    {
        parent::__construct(trim($name), $age);
    }
}
"#;

const PHP_PLANTS: &[Plant] = &[
    flagged("else-after-return", "src/Rules.php", (7, 9)),
    flagged("if-else-returns-boolean", "src/Rules.php", (22, 26)),
    flagged("ternary-yields-boolean", "src/Rules.php", (31, 31)),
    flagged("catch-only-rethrows", "src/Rules.php", (43, 45)),
    flagged("trailing-bare-return", "src/Rules.php", (61, 61)),
    flagged("constructor-only-forwards", "src/Rules.php", (74, 77)),
];

const DART_RULES: &str = r#"String sign(int value) {
  if (value < 0) {
    return 'negative';
  } else {
    return 'positive';
  }
}

String signClean(int value) {
  if (value < 0) {
    return 'negative';
  }
  return 'positive';
}

bool isAdult(int age) {
  if (age >= 18) {
    return true;
  } else {
    return false;
  }
}

bool ready(int count) => count > 3 ? true : false;

String label(int count) => count > 3 ? 'many' : 'few';

int load(int Function() read) {
  try {
    return read();
  } catch (error) {
    rethrow;
  }
}

int loadClean(int Function() read, void Function(Object) log) {
  try {
    return read();
  } catch (error) {
    log(error);
    rethrow;
  }
}

void notify(void Function() send) {
  send();
  return;
}

void notifyClean(void Function() send, bool enabled) {
  if (!enabled) {
    return;
  }
  send();
}
"#;

const DART_PLANTS: &[Plant] = &[
    flagged("else-after-return", "lib/rules.dart", (4, 6)),
    flagged("if-else-returns-boolean", "lib/rules.dart", (17, 21)),
    flagged("ternary-yields-boolean", "lib/rules.dart", (24, 24)),
    flagged("catch-only-rethrows", "lib/rules.dart", (31, 33)),
    flagged("trailing-bare-return", "lib/rules.dart", (47, 47)),
];

const PYTHON_RULES: &str = r#"def sign(value):
    if value < 0:
        return "negative"
    else:
        return "positive"


def sign_clean(value):
    if value < 0:
        return "negative"
    return "positive"


def is_adult(age):
    if age >= 18:
        return True
    else:
        return False


def is_adult_clean(age):
    return age >= 18


def ready(count):
    return True if count > 3 else False


def label(count):
    return "many" if count > 3 else "few"


def load(read):
    try:
        return read()
    except ValueError as error:
        raise error


def load_clean(read, log):
    try:
        return read()
    except ValueError as error:
        log(error)
        raise


def notify(send):
    send()
    return


def notify_clean(send, enabled):
    if not enabled:
        return
    send()
"#;

const PYTHON_PLANTS: &[Plant] = &[
    flagged("else-after-return", "app/rules.py", (4, 5)),
    flagged("if-else-returns-boolean", "app/rules.py", (15, 18)),
    flagged("conditional-yields-boolean", "app/rules.py", (26, 26)),
    flagged("except-only-reraises", "app/rules.py", (36, 37)),
    flagged("trailing-bare-return", "app/rules.py", (50, 50)),
];

const GO_RULES: &str = r#"package rules

func Sign(value int) string {
	if value < 0 {
		return "negative"
	} else {
		return "positive"
	}
}

func SignClean(value int) string {
	if value < 0 {
		return "negative"
	}
	return "positive"
}

func IsAdult(age int) bool {
	if age >= 18 {
		return true
	}
	return false
}

func Ready(count int) bool {
	var ready bool
	if count > 3 {
		ready = true
	} else {
		ready = false
	}
	return ready
}

func ReadyClean(count int) bool {
	return count > 3
}

func Notify(send func()) {
	send()
	return
}

func NotifyClean(send func(), enabled bool) {
	if !enabled {
		return
	}
	send()
}

func Route(kind int, send func()) {
	switch kind {
	case 1:
		send()
		break
	}
}

func RouteClean(kind int, send func()) {
	for {
		switch kind {
		case 1:
			send()
		}
		break
	}
}

func Load(read func() (int, error)) (int, error) {
	value, err := read()
	if err != nil {
		return 0, err
	}
	return value, nil
}
"#;

const GO_PLANTS: &[Plant] = &[
    flagged("else-after-return", "rules.go", (6, 8)),
    flagged("if-else-returns-boolean", "rules.go", (19, 22)),
    flagged("conditional-yields-boolean", "rules.go", (27, 31)),
    flagged("trailing-bare-return", "rules.go", (41, 41)),
    flagged("redundant-break", "rules.go", (55, 55)),
];

const SQL_STATEMENTS: &str = r"SELECT o.id, o.total FROM orders AS o WHERE o.status = 'open' OR o.status = 'held' OR o.status = 'late' LIMIT 10;
SELECT o.status, COUNT(o.id) FROM orders AS o WHERE o.id > 0 GROUP BY o.status HAVING o.status <> 'void' LIMIT 10;
SELECT * FROM orders AS o WHERE o.id = 1;
SELECT o.id FROM orders AS o WHERE o.note LIKE '%late' LIMIT 10;
SELECT o.id FROM orders AS o WHERE o.id > 0 LIMIT 10 OFFSET 5000;
SELECT o.id, c.name FROM orders AS o CROSS JOIN customers AS c WHERE o.id = 1 LIMIT 10;
SELECT DISTINCT o.status FROM orders AS o WHERE o.id > 0 ORDER BY o.status LIMIT 10;
SELECT o.id, (SELECT MAX(p.amount) FROM payments AS p WHERE p.id = 1) AS top FROM orders AS o WHERE o.id = 1;
SELECT o.id FROM orders AS o WHERE LOWER(o.status) = 'open' LIMIT 10;
SELECT o.id FROM orders AS o WHERE o.id NOT IN (SELECT r.order_id FROM refunds AS r WHERE r.id > 0) LIMIT 10;
SELECT o.id FROM orders AS o WHERE o.id = 1 UNION SELECT a.id FROM archived AS a WHERE a.id = 1;
SELECT o.id FROM orders AS o;
SELECT COUNT(*) FROM orders AS o LIMIT 1;
SELECT o.id FROM orders AS o WHERE o.id > 0 ORDER BY RANDOM() LIMIT 10;
SELECT DISTINCT o.id FROM orders AS o JOIN customers AS c ON c.id = o.customer_id WHERE o.id > 0 LIMIT 10;
SELECT o.id FROM orders AS o JOIN orders AS p ON p.id = o.parent_id WHERE o.id > 0 LIMIT 10;
SELECT o.id FROM orders AS o WHERE EXISTS (SELECT 1 FROM refunds AS r WHERE r.order_id = o.id) LIMIT 10;
SELECT o.id FROM orders AS o WHERE o.id IN (1, 2, 3) LIMIT 10;
SELECT o.id FROM orders AS o WHERE o.id IN (SELECT r.order_id FROM refunds AS r WHERE r.id IN (SELECT x.id FROM refund_items AS x WHERE x.id IN (SELECT y.item_id FROM items AS y WHERE y.id > 0))) LIMIT 10;
SELECT orders.id FROM orders JOIN customers AS c ON c.id = orders.customer_id WHERE orders.id > 0 LIMIT 10;
SELECT o.id FROM orders AS o WHERE o.id > 0 ORDER BY 1 LIMIT 10;
UPDATE orders SET status = 'void';
DELETE FROM orders;
TRUNCATE TABLE orders;
DROP TABLE orders;
GRANT SELECT ON orders TO reporter;
SELECT o.id FROM orders AS o WHERE o.id > 0 OR 1 = 1 LIMIT 10;
EXECUTE IMMEDIATE 'SELECT 1';
INSERT INTO accounts (id, password) VALUES (1, 'hunter2');
SELECT o.id, o.total FROM orders AS o WHERE o.id = 7 LIMIT 1;
SELECT o.id FROM orders AS o WHERE o.id IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101) LIMIT 10;
";

const SQL_SCHEMA: &str = r"CREATE TABLE shipments (id INTEGER PRIMARY KEY, carrier_id INTEGER, code TEXT, weight INTEGER);
CREATE INDEX shipments_code ON shipments (code);
CREATE TABLE carriers (carrier_id INTEGER PRIMARY KEY, name TEXT);
SELECT s.id FROM shipments AS s WHERE s.weight = 5 LIMIT 10;
SELECT s.id FROM shipments AS s WHERE s.id > 0 ORDER BY s.weight LIMIT 10;
SELECT s.id, c.name FROM shipments AS s JOIN carriers AS c USING (carrier_id) WHERE s.id = 1 LIMIT 10;
SELECT s.id FROM shipments AS s WHERE s.colour = 'red' AND s.id = 1 LIMIT 10;
SELECT s.id FROM shipments AS s WHERE s.code = 7 LIMIT 10;
SELECT s.id FROM shipments AS s WHERE s.code = 'A7' ORDER BY s.id LIMIT 10;
";

const SQL_PLANTS: &[Plant] = &[
    flagged("or-chain-collapses-to-in", "db/statements.sql", (1, 1)),
    flagged("having-without-aggregate", "db/statements.sql", (2, 2)),
    hazard("select-star", 3),
    hazard("leading-wildcard-like", 4),
    hazard("large-offset", 5),
    hazard("tables-without-join-condition", 6),
    hazard("distinct-with-order-by", 7),
    hazard("subquery-in-select-list", 8),
    hazard("function-on-filter-column", 9),
    hazard("not-in-subquery", 10),
    hazard("union-without-all", 11),
    hazard("select-without-filter-or-limit", 12),
    hazard("count-without-filter", 13),
    hazard("order-by-random", 14),
    hazard("distinct-over-join", 15),
    hazard("table-read-twice", 16),
    hazard("subquery-refers-to-outer-table", 17),
    hazard("deep-subquery-nesting", 19),
    hazard("join-without-aliases", 20),
    hazard("column-position", 21),
    hazard("update-without-filter", 22),
    hazard("delete-without-filter", 23),
    hazard("truncate", 24),
    hazard("drop", 25),
    hazard("grant-or-revoke", 26),
    hazard("always-true-condition", 27),
    hazard("sql-built-at-run-time", 28),
    hazard("plain-text-secret", 29),
    hazard("long-in-list", 31),
    schema_hazard("filter-on-unindexed-column", 4),
    schema_hazard("order-by-unindexed-column", 5),
    schema_hazard("join-on-unindexed-column", 6),
    schema_hazard("unknown-column", 7),
    schema_hazard("text-compared-to-number", 8),
];

const SHELL_FLAGGED: &str = r#"#!/bin/bash
set -euo pipefail
cat "$file" | sort
now=`date`
if [ $? -eq 0 ]; then echo ok; fi
decide() {
  if [ -f "$1" ]; then
    return 0
  else
    echo no
  fi
}
cat "$@" | sort
cat notes.txt another.txt | sort
now="$(date)"
if [ "$?" -eq 2 ]; then echo special; fi
decide_clean() {
  if [ -f "$1" ]; then
    echo yes
  else
    echo no
  fi
}
"#;

const SHELL_HAZARDS: &str = r#"#!/bin/bash
set -euo pipefail
echo $msg
eval "$cmd"
curl -fsSL https://example.invalid/i.sh | sh
rm -rf "$BUILD_DIR/out"
API_KEY="hunter2hunter2"
read line
curl -k https://example.invalid
echo "$msg"
eval "$(ssh-agent -s)"
curl -fsSL https://example.invalid/a.tgz | tar -xz
rm -rf "${BUILD_DIR:?}/out"
TOKEN_TYPE=bearer
SECRET_LABEL="Production DB"
systemctl restart disk-usage-monitor-daemon
read -r line
curl -fsSL https://example.invalid
cat <<$EOF
body
$EOF
"#;

const SHELL_NO_STRICT: &str = "#!/bin/sh\necho hi\n";

const SHELL_UNCHECKED_CD: &str = "#!/bin/bash\nset -u\ncd /tmp\ncd /var || exit 1\n";

const SHELL_BASHISM: &str =
    "#!/bin/sh\nset -eu\nif [[ -f a ]]; then echo ok; fi\nif [ -f a ]; then echo ok; fi\n";

const SHELL_MAIN_STRICT: &str =
    "#!/bin/bash\nmain() { set -euo pipefail; cd /tmp; ls; }\nmain \"$@\"\n";

const SHELL_ZSH: &str =
    "#!/bin/zsh\nsetopt err_exit no_unset pipe_fail\nprintf '%s' $words\nprintf '%s' $(date)\n";

const SHELL_PLANTS: &[Plant] = &[
    flagged("useless-cat", "scripts/flagged.bash", (3, 3)),
    flagged("legacy-backticks", "scripts/flagged.bash", (4, 4)),
    flagged(
        "redundant-exit-status-check",
        "scripts/flagged.bash",
        (5, 5),
    ),
    flagged("else-after-return", "scripts/flagged.bash", (9, 11)),
    hazard_in("unquoted-variable", "scripts/hazards.bash", 3),
    hazard_in("eval-with-variable", "scripts/hazards.bash", 4),
    hazard_in("pipe-download-to-shell", "scripts/hazards.bash", 5),
    hazard_in("unguarded-rm-rf", "scripts/hazards.bash", 6),
    hazard_in("hardcoded-secret", "scripts/hazards.bash", 7),
    hazard_in("read-without-r", "scripts/hazards.bash", 8),
    hazard_in("insecure-tls-download", "scripts/hazards.bash", 9),
    hazard_in("missing-strict-mode", "scripts/no_strict.sh", 2),
    hazard_in("missing-strict-mode", "scripts/unchecked_cd.bash", 2),
    hazard_in("unchecked-cd", "scripts/unchecked_cd.bash", 3),
    hazard_in("posix-sh-bashism", "scripts/bashism.sh", 3),
    hazard_in("unquoted-variable", "scripts/split.zsh", 4),
];

const RUST_MANIFEST: &str =
    "[package]\nname = \"planted\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

const RULE_SUITES: [RuleSuite; 8] = [
    RuleSuite {
        plugin: "typescript-javascript",
        files: &[("src/rules.ts", TYPESCRIPT_RULES)],
        plants: TYPESCRIPT_PLANTS,
    },
    RuleSuite {
        plugin: "rust",
        files: &[("Cargo.toml", RUST_MANIFEST), ("src/lib.rs", RUST_RULES)],
        plants: RUST_PLANTS,
    },
    RuleSuite {
        plugin: "php",
        files: &[("src/Rules.php", PHP_RULES)],
        plants: PHP_PLANTS,
    },
    RuleSuite {
        plugin: "dart",
        files: &[("lib/rules.dart", DART_RULES)],
        plants: DART_PLANTS,
    },
    RuleSuite {
        plugin: "python",
        files: &[("app/rules.py", PYTHON_RULES)],
        plants: PYTHON_PLANTS,
    },
    RuleSuite {
        plugin: "go",
        files: &[("rules.go", GO_RULES)],
        plants: GO_PLANTS,
    },
    RuleSuite {
        plugin: "sql",
        files: &[
            ("db/statements.sql", SQL_STATEMENTS),
            ("db/schema.sql", SQL_SCHEMA),
        ],
        plants: SQL_PLANTS,
    },
    RuleSuite {
        plugin: "shell",
        files: &[
            ("scripts/flagged.bash", SHELL_FLAGGED),
            ("scripts/hazards.bash", SHELL_HAZARDS),
            ("scripts/no_strict.sh", SHELL_NO_STRICT),
            ("scripts/unchecked_cd.bash", SHELL_UNCHECKED_CD),
            ("scripts/bashism.sh", SHELL_BASHISM),
            ("scripts/main_strict.bash", SHELL_MAIN_STRICT),
            ("scripts/split.zsh", SHELL_ZSH),
        ],
        plants: SHELL_PLANTS,
    },
];

#[test]
fn planted_typescript_flagged_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[0]);
}

#[test]
fn planted_rust_flagged_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[1]);
}

#[test]
fn planted_php_flagged_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[2]);
}

#[test]
fn planted_dart_flagged_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[3]);
}

#[test]
fn planted_python_flagged_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[4]);
}

#[test]
fn planted_go_flagged_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[5]);
}

#[test]
fn planted_sql_flagged_and_hazard_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[6]);
}

#[test]
fn planted_shell_flagged_and_hazard_lines_are_all_found_without_false_positives() {
    run_rule_suite(&RULE_SUITES[7]);
}

#[test]
fn planted_hazard_lines_cover_every_declared_hazard_line_rule() {
    let planted: BTreeSet<String> = RULE_SUITES
        .iter()
        .flat_map(|suite| suite.plants)
        .filter(|plant| plant.metric == "hazards")
        .map(|plant| plant.rule.to_owned())
        .collect();
    let declared: BTreeSet<String> = built_in_language_hazard_rules()
        .into_values()
        .flatten()
        .collect();
    assert_eq!(planted, declared);
}

const METRIC_PACKAGE_JSON: &str = r#"{
  "name": "planted-metrics",
  "version": "1.0.0",
  "main": "src/index.ts",
  "dependencies": {
    "left-pad": "1.3.0",
    "zod": "3.23.8"
  },
  "devDependencies": {
    "chalk": "5.3.0"
  }
}
"#;

const METRIC_CODEINT_JSON: &str = r#"{
  "boundaries": {
    "zones": {
      "ui": { "patterns": ["src/ui/**"] },
      "db": { "patterns": ["src/db/**"] },
      "services": { "patterns": ["src/services/**"] }
    },
    "rules": [
      { "from": "ui", "allow": ["services"], "reason": "Views reach storage through services." }
    ]
  }
}
"#;

const METRIC_INDEX: &str = r#"import { z } from "zod";
import chalk from "chalk";
import { merge } from "lodash";
import { first } from "./cycle/first";
import { render, renderClean } from "./ui/view";
import { grade, gradeClean } from "./grade";
import { nested, nestedClean } from "./nested";
import { total } from "./total";
import { sum } from "./sum";
import { covered, untested } from "./covered";

export const schema = z.string();
export const banner = chalk.bold(merge({}, { title: "planted" }).title);
export const results = [
  first(1),
  render(),
  renderClean(),
  grade(50),
  gradeClean(50, 1),
  nested(1, 2, 3, 4, 5, 6),
  nestedClean(1, 2, 3, 4, 5),
  total([1]),
  sum([2]),
  covered(3),
  untested(4),
];
"#;

const METRIC_CYCLE_FIRST: &str = r#"import { second } from "./second";

export function first(value: number): number {
  return value > 0 ? second(value - 1) : 0;
}
"#;

const METRIC_CYCLE_SECOND: &str = r#"import { first } from "./first";

export function second(value: number): number {
  return value > 0 ? first(value - 1) : 1;
}
"#;

const METRIC_VIEW: &str = r#"import { load } from "../db/store";
import { fetchRow } from "../services/rows";

export function render(): string {
  return load().toUpperCase();
}

export function renderClean(): string {
  return fetchRow().toLowerCase();
}
"#;

const METRIC_SERVICE: &str = r#"import { load } from "../db/store";

export function fetchRow(): string {
  return load().trim();
}
"#;

const METRIC_STORE: &str = r#"export function load(): string {
  return "row";
}
"#;

const METRIC_ORPHAN: &str = r#"export function forgotten(): number {
  return 42;
}
"#;

const METRIC_GRADE: &str = r#"export function grade(score: number): string {
  if (score > 95) return "A+";
  if (score > 90) return "A";
  if (score > 85) return "A-";
  if (score > 80) return "B+";
  if (score > 75) return "B";
  if (score > 70) return "B-";
  if (score > 65) return "C+";
  if (score > 60) return "C";
  if (score > 55) return "C-";
  if (score > 50) return "D+";
  if (score > 45) return "D";
  return "F";
}

export function gradeClean(score: number, bonus: number): string {
  const adjusted = bonus > 0 ? score + bonus : score;
  const thresholds = [90, 80, 70, 60];
  for (const threshold of thresholds) {
    if (adjusted > threshold && threshold > 50) {
      return threshold > 85 || bonus > 9 ? "high" : "mid";
    }
  }
  while (adjusted < 0 && bonus < 0) {
    return "invalid";
  }
  return score === 0 ? "none" : "low";
}
"#;

const METRIC_NESTED: &str = r"export function nested(a: number, b: number, c: number, d: number, e: number, f: number): number {
  if (a > 0) {
    if (b > 0) {
      if (c > 0) {
        if (d > 0) {
          if (e > 0) {
            if (f > 0) {
              return a + b + c + d + e + f;
            }
          }
        }
      }
    }
  }
  return 0;
}

export function nestedClean(a: number, b: number, c: number, d: number, e: number): number {
  if (a > 0) {
    if (b > 0) {
      if (c > 0) {
        if (d > 0) {
          if (e > 0) {
            return a * b * c * d * e;
          }
        }
      }
    }
  }
  return 1;
}
";

const METRIC_TOTAL: &str = r"export function total(prices: number[]): number {
  let result = 0;
  for (const price of prices) {
    const taxed = price * 1.2;
    const discounted = taxed > 100 ? taxed - 5 : taxed;
    const rounded = Math.round(discounted * 100) / 100;
    if (rounded < 0) {
      throw new Error('negative price ' + rounded);
    }
    result = result + rounded;
  }
  return result;
}
";

const METRIC_SUM: &str = r"export function sum(amounts: number[]): number {
  let value = 0;
  for (const amount of amounts) {
    const scaled = amount * 3.5;
    const reduced = scaled > 200 ? scaled - 9 : scaled;
    const trimmed = Math.floor(reduced * 10) / 10;
    if (trimmed < 0) {
      throw new Error('negative amount ' + trimmed);
    }
    value = value + trimmed;
  }
  return value;
}
";

const METRIC_COVERED: &str = r"export function covered(value: number): number {
  return value * 2;
}

export function untested(value: number): number {
  return value * 3;
}
";

const METRIC_FILES: &[(&str, &str)] = &[
    ("package.json", METRIC_PACKAGE_JSON),
    ("codeint.json", METRIC_CODEINT_JSON),
    ("src/index.ts", METRIC_INDEX),
    ("src/cycle/first.ts", METRIC_CYCLE_FIRST),
    ("src/cycle/second.ts", METRIC_CYCLE_SECOND),
    ("src/ui/view.ts", METRIC_VIEW),
    ("src/services/rows.ts", METRIC_SERVICE),
    ("src/db/store.ts", METRIC_STORE),
    ("src/orphan.ts", METRIC_ORPHAN),
    ("src/grade.ts", METRIC_GRADE),
    ("src/nested.ts", METRIC_NESTED),
    ("src/total.ts", METRIC_TOTAL),
    ("src/sum.ts", METRIC_SUM),
    ("src/covered.ts", METRIC_COVERED),
];

const UNTESTED_LINES: (&str, u32, u32) = ("src/covered.ts", 5, 7);

const METRIC_PLANTS: &[(&str, &str)] = &[
    ("erosion", "src/grade.ts:1"),
    ("cognitive_complexity", "src/nested.ts:1"),
    ("duplication", "src/sum.ts:1-13 src/total.ts:1-13"),
    ("import_cycles", "src/cycle/first.ts src/cycle/second.ts"),
    ("boundaries", "src/ui/view.ts -> src/db/store.ts"),
    ("orphan_files", "src/orphan.ts"),
    ("test_gap", "src/covered.ts:5"),
    ("unused_dependencies", "left-pad"),
    ("unlisted_dependencies", "lodash"),
    ("dev_in_production_dependencies", "chalk"),
];

const LINE_RULE_METRICS: [&str; 2] = ["verbosity", "hazards"];

fn full_coverage_except_untested() -> String {
    let mut lcov = String::new();
    for (path, text) in METRIC_FILES
        .iter()
        .filter(|(path, _)| path.ends_with(".ts"))
    {
        writeln!(lcov, "SF:{path}").expect("write to string");
        for line in 1..=u32::try_from(text.lines().count()).expect("small file") {
            let untested =
                *path == UNTESTED_LINES.0 && (UNTESTED_LINES.1..=UNTESTED_LINES.2).contains(&line);
            writeln!(lcov, "DA:{line},{}", u32::from(!untested)).expect("write to string");
        }
        lcov.push_str("end_of_record\n");
    }
    lcov
}

fn joined(values: &Value, field: Option<&str>) -> String {
    let parts: Vec<String> = values
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| match field {
            Some(_) => format!(
                "{}:{}-{}",
                value["path"].as_str().unwrap_or_default(),
                value["start_line"],
                value["end_line"]
            ),
            None => value.as_str().unwrap_or_default().to_owned(),
        })
        .collect();
    parts.join(" ")
}

fn metric_finding_identity(metric: &str, finding: &Value) -> String {
    match metric {
        "duplication" => joined(&finding["locations"], Some("path")),
        "import_cycles" => joined(&finding["modules"], None),
        "boundaries" => format!(
            "{} -> {}",
            finding["importer"].as_str().unwrap_or_default(),
            finding["target"].as_str().unwrap_or_default()
        ),
        "orphan_files" => finding["path"].as_str().unwrap_or_default().to_owned(),
        "unused_dependencies" | "unlisted_dependencies" | "dev_in_production_dependencies" => {
            finding["dependency"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        }
        _ => format!(
            "{}:{}",
            finding["path"].as_str().unwrap_or_default(),
            finding["line"]
        ),
    }
}

fn metric_section<'report>(report: &'report Value, metric: &str) -> &'report Value {
    let production = &report["source_sets"]["production"][metric];
    if production.is_null() {
        &report["dependencies"][metric]
    } else {
        production
    }
}

fn reported_metric_findings(report: &Value) -> BTreeSet<(String, String)> {
    let mut found = BTreeSet::new();
    for metric in Metric::BUILT_IN_KEYS
        .into_iter()
        .filter(|metric| !LINE_RULE_METRICS.contains(metric))
    {
        let section = metric_section(report, metric);
        let list = if metric == "duplication" {
            &section["clone_groups"]
        } else {
            &section["findings"]
        };
        for finding in list.as_array().into_iter().flatten() {
            found.insert((metric.to_owned(), metric_finding_identity(metric, finding)));
        }
    }
    found
}

#[test]
fn planted_metric_defects_are_all_found_without_false_positives() {
    let repo = init_git_repo(METRIC_FILES);
    let lcov = full_coverage_except_untested();
    std::fs::write(repo.path().join("lcov.info"), lcov).expect("write coverage report");
    let report = check_all_json(repo.path(), &["--coverage-report", "lcov.info"]);
    let expected: BTreeSet<(String, String)> = METRIC_PLANTS
        .iter()
        .map(|(metric, identity)| ((*metric).to_owned(), (*identity).to_owned()))
        .collect();
    assert_full_recall_and_no_false_positives(
        "metrics",
        &expected,
        &reported_metric_findings(&report),
    );
    assert_full_recall_and_no_false_positives(
        "metrics repository line rules",
        &BTreeSet::new(),
        &reported_line_findings(&report),
    );
}

#[test]
fn planted_defects_cover_every_built_in_metric() {
    let planted: BTreeSet<&str> = METRIC_PLANTS
        .iter()
        .map(|(metric, _)| *metric)
        .chain(LINE_RULE_METRICS)
        .collect();
    let declared: BTreeSet<&str> = Metric::BUILT_IN_KEYS.into_iter().collect();
    assert_eq!(planted, declared);
}
