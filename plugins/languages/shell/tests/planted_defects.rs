use codeint_lang_shell::Shell;
use codeint_model::{LanguagePlugin, RepoPath};

fn lower(path: &str, src: &str) -> codeint_model::FileModel {
    Shell
        .lower(&RepoPath::new(path).unwrap(), src)
        .unwrap_or_else(|err| panic!("failed to lower {path}: {err}"))
}

fn flagged_lines_for(path: &str, src: &str, rule: &str) -> Vec<u32> {
    lower(path, src)
        .flagged_lines
        .into_iter()
        .filter(|f| f.rule == rule)
        .map(|f| f.line)
        .collect()
}

fn hazard_lines_for(path: &str, src: &str, rule: &str) -> Vec<u32> {
    lower(path, src)
        .hazard_lines
        .into_iter()
        .filter(|h| h.rule == rule)
        .map(|h| h.line)
        .collect()
}

fn assert_flagged_rule(rule: &str, hits: &[(&str, &str, &[u32])], misses: &[(&str, &str)]) {
    for &(path, src, expected) in hits {
        let got = flagged_lines_for(path, src, rule);
        assert_eq!(
            got, expected,
            "flagged rule {rule} mismatch on {path}:\n{src}"
        );
    }
    for &(path, src) in misses {
        let got = flagged_lines_for(path, src, rule);
        assert!(
            got.is_empty(),
            "flagged rule {rule} false positive at {got:?} on {path}:\n{src}"
        );
    }
}

fn assert_hazard_rule(rule: &str, hits: &[(&str, &str, &[u32])], misses: &[(&str, &str)]) {
    for &(path, src, expected) in hits {
        let got = hazard_lines_for(path, src, rule);
        assert_eq!(
            got, expected,
            "hazard rule {rule} mismatch on {path}:\n{src}"
        );
    }
    for &(path, src) in misses {
        let got = hazard_lines_for(path, src, rule);
        assert!(
            got.is_empty(),
            "hazard rule {rule} false positive at {got:?} on {path}:\n{src}"
        );
    }
}

#[test]
fn planted_defects_flagged_line_rules_achieve_full_recall_and_zero_false_positives() {
    assert_flagged_rule(
        "useless-cat",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\ncat notes.txt | grep foo\n",
                &[3],
            ),
            (
                "b.bash",
                "#!/bin/bash\nset -euo pipefail\nx=\"$(cat \"$file\" | wc -l)\"\n",
                &[3],
            ),
        ],
        &[
            ("c.sh", "#!/bin/sh\nset -eu\ncat a.txt b.txt | grep foo\n"),
            ("d.sh", "#!/bin/sh\nset -eu\ncat -n notes.txt | grep foo\n"),
            ("e.sh", "#!/bin/sh\nset -eu\ncat *.log | grep err\n"),
            ("f.sh", "#!/bin/sh\nset -eu\ncat - | grep foo\n"),
            ("g.sh", "#!/bin/sh\nset -eu\ngrep foo notes.txt\n"),
            (
                "h.sh",
                "#!/bin/sh\nset -eu\ncat <<EOF | grep foo\nhello\nEOF\n",
            ),
        ],
    );

    assert_flagged_rule(
        "legacy-backticks",
        &[
            ("a.sh", "#!/bin/sh\nset -eu\nnow=`date`\n", &[3]),
            ("b.sh", "#!/bin/sh\nset -eu\nmsg=\"hello `whoami`\"\n", &[3]),
        ],
        &[
            ("c.sh", "#!/bin/sh\nset -eu\nnow=\"$(date)\"\n"),
            ("d.sh", "#!/bin/sh\nset -eu\necho '`literal`'\n"),
            (
                "e.sh",
                "#!/bin/sh\nset -eu\necho \"escaped \\`backtick\\`\"\n",
            ),
            ("f.sh", "#!/bin/sh\nset -eu\n# comment `ignored`\necho ok\n"),
        ],
    );

    assert_flagged_rule(
        "redundant-exit-status-check",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\nif [ $? -eq 0 ]; then echo ok; fi\n",
                &[3],
            ),
            (
                "b.bash",
                "#!/bin/bash\nset -euo pipefail\nif [[ $? -ne 0 ]]; then exit 1; fi\n",
                &[3],
            ),
            (
                "c.sh",
                "#!/bin/sh\nset -eu\nif test \"$?\" -eq 0; then echo ok; fi\n",
                &[3],
            ),
            (
                "d.sh",
                "#!/bin/sh\nset -eu\nif [ 0 -eq $? ]; then echo ok; fi\n",
                &[3],
            ),
        ],
        &[
            (
                "e.sh",
                "#!/bin/sh\nset -eu\nif grep -q foo bar; then echo ok; fi\n",
            ),
            (
                "f.sh",
                "#!/bin/sh\nset -eu\nif [ $? -eq 2 ]; then echo special; fi\n",
            ),
            ("g.sh", "#!/bin/sh\nset -eu\nstatus=$?\necho \"$status\"\n"),
        ],
    );

    assert_flagged_rule(
        "else-after-return",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\nf() {\n  if [ -f \"a\" ]; then\n    return 0\n  else\n    echo \"no\"\n  fi\n}\n",
                &[6, 7, 8],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\nif [ -f \"a\" ]; then\n  exit 1\nelse\n  echo \"ok\"\nfi\n",
                &[5, 6, 7],
            ),
        ],
        &[
            (
                "c.sh",
                "#!/bin/sh\nset -eu\nf() {\n  if [ -f \"a\" ]; then\n    echo \"yes\"\n  else\n    echo \"no\"\n  fi\n}\n",
            ),
            (
                "d.sh",
                "#!/bin/sh\nset -eu\nf() {\n  if [ -f \"a\" ]; then\n    test -f \"b\" && return 0\n  else\n    echo \"no\"\n  fi\n}\n",
            ),
            (
                "e.sh",
                "#!/bin/sh\nset -eu\nf() {\n  if [ -f \"a\" ]; then\n    if [ -f \"b\" ]; then\n      return 0\n    fi\n  else\n    echo \"no\"\n  fi\n}\n",
            ),
        ],
    );
}

#[test]
fn planted_defects_strict_mode_unquoted_var_eval_and_pipe_download() {
    assert_hazard_rule(
        "missing-strict-mode",
        &[
            ("a.sh", "#!/bin/sh\necho hi\n", &[2]),
            ("b.bash", "#!/usr/bin/env bash\nset -eu\necho hi\n", &[2]),
            ("c.zsh", "#!/usr/bin/env zsh\nset -e\necho hi\n", &[2]),
            ("d.sh", "#!/bin/sh\nset -e\necho hi\n", &[2]),
        ],
        &[
            ("ok.sh", "#!/bin/sh\nset -eu\necho hi\n"),
            ("ok2.sh", "#!/bin/sh -eu\necho hi\n"),
            (
                "ok.bash",
                "#!/usr/bin/env bash\nset -euo pipefail\necho hi\n",
            ),
            (
                "ok2.bash",
                "#!/bin/bash\nset -o errexit\nset -o nounset\nset -o pipefail\necho hi\n",
            ),
            (
                "ok.zsh",
                "#!/bin/zsh\nsetopt err_exit no_unset pipe_fail\necho hi\n",
            ),
            ("lib.sh", "#!/bin/sh\nhelper() {\n  echo hi\n}\n"),
            ("empty.sh", "#!/bin/sh\n# nothing\n"),
        ],
    );

    assert_hazard_rule(
        "unquoted-variable",
        &[
            ("a.sh", "#!/bin/sh\nset -eu\necho $msg\n", &[3]),
            ("b.sh", "#!/bin/sh\nset -eu\nrm ${target}\n", &[3]),
            ("c.sh", "#!/bin/sh\nset -eu\ncat \"$(echo $inner)\"\n", &[3]),
            ("d.sh", "#!/bin/sh\nset -eu\necho hi > $outfile\n", &[3]),
            (
                "e.sh",
                "#!/bin/sh\nset -eu\nfor x in $items; do echo \"$x\"; done\n",
                &[3],
            ),
        ],
        &[
            ("q1.sh", "#!/bin/sh\nset -eu\necho \"$msg\"\n"),
            (
                "q2.sh",
                "#!/bin/sh\nset -eu\nx=$other\nexport PATH=$HOME/bin:$PATH\n",
            ),
            (
                "q3.bash",
                "#!/bin/bash\nset -euo pipefail\nif [[ -f $file && $a == \"b\" ]]; then echo ok; fi\n",
            ),
            (
                "q4.bash",
                "#!/bin/bash\nset -euo pipefail\n(( x = y + 1 ))\necho \"$(( $x + 1 ))\"\n",
            ),
            (
                "q5.sh",
                "#!/bin/sh\nset -eu\ncase $mode in prod) echo 1 ;; esac\n",
            ),
            ("q6.sh", "#!/bin/sh\nset -eu\necho $? $$ $! $# ${#msg}\n"),
            ("q7.sh", "#!/bin/sh\nset -eu\necho '$literal'\n"),
        ],
    );

    assert_hazard_rule(
        "eval-with-variable",
        &[
            ("a.sh", "#!/bin/sh\nset -eu\neval \"$cmd\"\n", &[3]),
            ("b.sh", "#!/bin/sh\nset -eu\neval \"echo ${name}\"\n", &[3]),
        ],
        &[
            ("c.sh", "#!/bin/sh\nset -eu\neval 'echo safe'\n"),
            ("d.sh", "#!/bin/sh\nset -eu\necho eval \"$cmd\"\n"),
        ],
    );

    assert_hazard_rule(
        "pipe-download-to-shell",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\ncurl -fsSL https://example.com/i.sh | sh\n",
                &[3],
            ),
            (
                "b.bash",
                "#!/bin/bash\nset -euo pipefail\nwget -qO- https://example.com/i.sh | sudo bash\n",
                &[3],
            ),
            (
                "c.bash",
                "#!/bin/bash\nset -euo pipefail\nbash <(curl -fsSL https://example.com/i.sh)\n",
                &[3],
            ),
        ],
        &[
            (
                "d.sh",
                "#!/bin/sh\nset -eu\ncurl -fsSL https://example.com/a.tgz | tar -xz\n",
            ),
            (
                "e.sh",
                "#!/bin/sh\nset -eu\ncurl -fsSL https://example.com/i.sh -o i.sh\n",
            ),
            (
                "f.sh",
                "#!/bin/sh\nset -eu\necho 'curl https://example.com | sh'\n",
            ),
        ],
    );
}

#[test]
fn planted_defects_cd_rm_secrets_read_bashisms_and_tls() {
    assert_hazard_rule(
        "unchecked-cd",
        &[
            ("a.sh", "#!/bin/sh\ncd \"$dir\"\necho hi\n", &[2]),
            ("b.bash", "#!/bin/bash\nset -u\ncd /tmp\n", &[3]),
        ],
        &[
            ("c.sh", "#!/bin/sh\nset -eu\ncd \"$dir\"\n"),
            ("d.sh", "#!/bin/sh\ncd \"$dir\" || exit 1\n"),
            ("e.sh", "#!/bin/sh\ncd \"$dir\" && ls\n"),
            ("f.sh", "#!/bin/sh\nif cd \"$dir\"; then ls; fi\n"),
        ],
    );

    assert_hazard_rule(
        "unguarded-rm-rf",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\nrm -rf \"$BUILD_DIR/out\"\n",
                &[3],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\nrm -r -f \"${TMP_DIR}/cache\"\n",
                &[3],
            ),
            (
                "c.sh",
                "#!/bin/sh\nset -eu\nsudo rm -rf \"$TARGET\"/*\n",
                &[3],
            ),
            ("d.sh", "#!/bin/sh\nset -eu\nrm -fr \"/$DIR/sub\"\n", &[3]),
        ],
        &[
            (
                "e.sh",
                "#!/bin/sh\nset -eu\nrm -rf \"${BUILD_DIR:?}/out\"\n",
            ),
            (
                "f.sh",
                "#!/bin/sh\nset -eu\nrm -rf \"${TMP_DIR:?missing}\"/*\n",
            ),
            ("g.sh", "#!/bin/sh\nset -eu\nrm -rf ./dist\n"),
            ("h.sh", "#!/bin/sh\nset -eu\nrm -f \"$TEMP_FILE\"\n"),
        ],
    );

    let secret_script = "\
#!/bin/sh
set -eu
API_KEY=\"live-secret-99887766\"
export DB_PASSWORD='super_secret_password'
TOKEN=\"ghp_1234567890abcdef1234567890\"
";
    assert_hazard_rule(
        "hardcoded-secret",
        &[
            ("secret.sh", secret_script, &[3, 4, 5]),
            (
                "aws.sh",
                "#!/bin/sh\nset -eu\necho \"AKIAIOSFODNN7EXAMPLE\"\n",
                &[3],
            ),
        ],
        &[
            ("env.sh", "#!/bin/sh\nset -eu\nAPI_KEY=\"$ENV_API_KEY\"\n"),
            (
                "path.sh",
                "#!/bin/sh\nset -eu\nPASSWORD_FILE=\"/run/secrets/db\"\n",
            ),
            (
                "placeholder.sh",
                "#!/bin/sh\nset -eu\nAPI_KEY=\"changeme\"\nDB_PASSWORD=\"<secret>\"\n",
            ),
            ("empty.sh", "#!/bin/sh\nset -eu\nTOKEN=\"\"\n"),
        ],
    );
    let model = lower("secret.sh", secret_script);
    for h in &model.hazard_lines {
        assert!(!h.rule.contains("99887766"));
        assert!(!h.rule.contains("super_secret"));
        assert!(!h.rule.contains("ghp_"));
    }

    assert_hazard_rule(
        "read-without-r",
        &[
            ("a.sh", "#!/bin/sh\nset -eu\nread line\n", &[3]),
            (
                "b.bash",
                "#!/bin/bash\nset -euo pipefail\nread -p \"Name: \" name\n",
                &[3],
            ),
        ],
        &[
            ("c.sh", "#!/bin/sh\nset -eu\nread -r line\n"),
            (
                "d.bash",
                "#!/bin/bash\nset -euo pipefail\nread -rs -p \"Pass: \" pw\n",
            ),
        ],
    );

    assert_hazard_rule(
        "posix-sh-bashism",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\nif [[ -f \"a\" ]]; then echo ok; fi\n",
                &[3],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\nfunction greet { echo hi; }\n",
                &[3],
            ),
            ("c.sh", "#!/bin/sh\nset -eu\n(( count += 1 ))\n", &[3]),
            ("d.sh", "#!/bin/sh\nset -eu\nread -r x <<< \"val\"\n", &[3]),
            ("e.sh", "#!/bin/sh\nset -eu\nsource ./env.sh\n", &[3]),
            ("f.sh", "#!/bin/sh\nset -eu\narr=(one two)\n", &[3]),
            ("g.sh", "#!/bin/sh\nset -eu\ncmd &> /dev/null\n", &[3]),
            ("h.sh", "#!/bin/sh\nset -eu\ndiff <(ls) <(ls)\n", &[3]),
        ],
        &[
            (
                "ok.sh",
                "#!/bin/sh\nset -eu\nif [ -f \"a\" ]; then echo $((1 + 2)); fi\n. ./env.sh\n",
            ),
            (
                "ok.bash",
                "#!/usr/bin/env bash\nset -euo pipefail\nfunction greet { if [[ -f \"a\" ]]; then (( x += 1 )); fi; }\narr=(one two)\nsource ./env.sh\n",
            ),
            (
                "ok.zsh",
                "#!/bin/zsh\nset -euo pipefail\nif [[ -f \"a\" ]]; then echo ok; fi\n",
            ),
        ],
    );

    assert_hazard_rule(
        "insecure-tls-download",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\ncurl -k https://example.com\n",
                &[3],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\ncurl -fsSLk https://example.com\n",
                &[3],
            ),
            (
                "c.sh",
                "#!/bin/sh\nset -eu\ncurl --insecure https://example.com\n",
                &[3],
            ),
            (
                "d.sh",
                "#!/bin/sh\nset -eu\nwget --no-check-certificate https://example.com\n",
                &[3],
            ),
        ],
        &[
            (
                "e.sh",
                "#!/bin/sh\nset -eu\ncurl -fsSL https://example.com\n",
            ),
            (
                "f.sh",
                "#!/bin/sh\nset -eu\ncurl -H \"X-Flag: -k\" https://example.com\n",
            ),
            ("g.sh", "#!/bin/sh\nset -eu\nwget -q https://example.com\n"),
        ],
    );
}
