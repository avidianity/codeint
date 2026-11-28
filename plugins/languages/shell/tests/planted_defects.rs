use codeint_lang_shell::Shell;
use codeint_model::{LanguagePlugin, RepoPath};

fn lower(path: &str, src: &str) -> codeint_model::FileModel {
    Shell
        .lower(&RepoPath::new(path).unwrap(), src, None)
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

#[test]
fn planted_defects_secret_prefixes_need_a_word_boundary() {
    assert_hazard_rule(
        "hardcoded-secret",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\nOPENAI=\"sk-abcdefghijklmnopqrstuvwx\"\n",
                &[3],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\nexport KEY=sk-proj-abcdefghijklmnop_qrstuv\n",
                &[3],
            ),
        ],
        &[
            (
                "m1.sh",
                "#!/bin/sh\nset -eu\nsystemctl restart disk-usage-monitor-daemon\n",
            ),
            (
                "m2.sh",
                "#!/bin/sh\nset -eu\npip install flask-sqlalchemy-utils-extra\n",
            ),
            (
                "m3.sh",
                "#!/bin/sh\nset -eu\ndocker run --name task-scheduler-worker-main app\n",
            ),
            (
                "m4.sh",
                "#!/bin/sh\nset -eu\nkubectl rollout restart deployment/risk-assessment-api-server\n",
            ),
            (
                "m5.sh",
                "#!/bin/sh\nset -eu\nname=sk-learn-pipeline-runner-tool\n",
            ),
        ],
    );
}

#[test]
fn planted_defects_secret_names_need_a_secret_looking_value() {
    assert_hazard_rule(
        "hardcoded-secret",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\nAPI_TOKEN=\"abcd-1234-efgh\"\n",
                &[3],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\nDB_PASSWORD=hunter2hunter2\n",
                &[3],
            ),
        ],
        &[
            ("m1.sh", "#!/bin/sh\nset -eu\nTOKEN_TYPE=bearer\n"),
            (
                "m2.sh",
                "#!/bin/sh\nset -eu\nSECRET_LABEL=\"Production DB\"\n",
            ),
            (
                "m3.sh",
                "#!/bin/sh\nset -eu\nTOKEN_ENDPOINT=https://auth.example.invalid/token\n",
            ),
            (
                "m4.sh",
                "#!/bin/sh\nset -eu\nPASSWORD_CMD=\"pass show db\"\n",
            ),
            (
                "m5.sh",
                "#!/bin/sh\nset -eu\nSECRETS_DIRECTORY=/etc/secrets\n",
            ),
            (
                "m6.sh",
                "#!/bin/sh\nset -eu\npassword_hint=\"Enter password\"\n",
            ),
            ("m7.sh", "#!/bin/sh\nset -eu\nsecret_count=100000\n"),
            ("m8.sh", "#!/bin/sh\nset -eu\nTOKEN_SCOPE=\"read:org\"\n"),
            (
                "m9.sh",
                "#!/bin/sh\nset -eu\nTOKEN_POLICY=rotate-quarterly\nAUTH_TOKEN=https://example.invalid/t\nAUTH_SECRET=/run/secrets/db\nAUTH_PASSWORD=12345678\n",
            ),
        ],
    );
}

#[test]
fn planted_defects_zsh_splits_only_command_substitutions_and_explicit_forms() {
    assert_hazard_rule(
        "unquoted-variable",
        &[
            ("a.zsh", "#!/bin/zsh\nset -eu\nprintf '%s' $(date)\n", &[3]),
            (
                "b.zsh",
                "#!/bin/zsh\nset -eu\nprintf '%s' ${=words}\n",
                &[3],
            ),
            (
                "c.zsh",
                "#!/bin/zsh\nset -eu\nprintf '%s' $~pattern\n",
                &[3],
            ),
            ("d.zsh", "#!/bin/zsh\nset -eu\nprintf '%s' `date`\n", &[3]),
            ("e.sh", "#!/bin/sh\nset -eu\nprintf '%s' $words\n", &[3]),
        ],
        &[
            ("m1.zsh", "#!/bin/zsh\nset -eu\nprintf '%s' $words\n"),
            ("m2.zsh", "#!/bin/zsh\nset -eu\nrm ${target} > $log\n"),
            (
                "m3.zsh",
                "#!/bin/zsh\nset -eu\nfor x in $items; do echo \"$x\"; done\n",
            ),
            ("m4.zsh", "#!/bin/zsh\nset -eu\nprintf '%s' \"${=words}\"\n"),
        ],
    );
}

#[test]
fn planted_defects_eval_needs_a_variable_not_just_a_command_substitution() {
    assert_hazard_rule(
        "eval-with-variable",
        &[
            (
                "a.sh",
                "#!/bin/sh\nset -eu\neval \"$(echo \"$cmd\")\"\n",
                &[3],
            ),
            (
                "b.sh",
                "#!/bin/sh\nset -eu\neval \"$(ssh-agent -s)\" \"$extra\"\n",
                &[3],
            ),
            (
                "c.sh",
                "#!/bin/sh\nset -eu\neval \"$((1 + count))\"\n",
                &[3],
            ),
            (
                "d.sh",
                "#!/bin/sh\nset -eu\neval \"`ssh-agent -s`\"\n",
                &[3],
            ),
        ],
        &[
            ("m1.sh", "#!/bin/sh\nset -eu\neval \"$(ssh-agent -s)\"\n"),
            (
                "m2.sh",
                "#!/bin/sh\nset -eu\neval \"$(direnv hook bash)\"\n",
            ),
            ("m3.sh", "#!/bin/sh\nset -eu\neval \"$(brew shellenv)\"\n"),
            ("m4.sh", "#!/bin/sh\nset -eu\neval $(brew shellenv)\n"),
        ],
    );
}

#[test]
fn planted_defects_useless_cat_ignores_argument_list_operands() {
    assert_flagged_rule(
        "useless-cat",
        &[("a.sh", "#!/bin/sh\nset -eu\ncat \"$file\" | sort\n", &[3])],
        &[
            ("m1.sh", "#!/bin/sh\nset -eu\ncat \"$@\" | sort\n"),
            ("m2.sh", "#!/bin/sh\nset -eu\ncat $* | sort\n"),
            (
                "m3.bash",
                "#!/bin/bash\nset -euo pipefail\ncat \"${files[@]}\" | sort\n",
            ),
            ("m4.sh", "#!/bin/sh\nset -eu\ncat \"${@}\" | sort\n"),
        ],
    );
}

#[test]
fn planted_defects_strict_mode_set_inside_a_called_function_counts() {
    assert_hazard_rule(
        "missing-strict-mode",
        &[
            (
                "a.bash",
                "#!/bin/bash\nmain() {\n  echo hi\n}\nmain \"$@\"\n",
                &[2],
            ),
            (
                "b.bash",
                "#!/bin/bash\nmain() {\n  set -euo pipefail\n  echo hi\n}\nother\n",
                &[2],
            ),
            (
                "c.bash",
                "#!/bin/bash\nmain() {\n  echo first\n  set -euo pipefail\n}\nmain\n",
                &[2],
            ),
            (
                "d.bash",
                "#!/bin/bash\nmain() {\n  set -eu\n  echo hi\n}\nmain\n",
                &[2],
            ),
        ],
        &[
            (
                "m1.bash",
                "#!/bin/bash\nmain() {\n  set -euo pipefail\n  cd /tmp\n}\nmain \"$@\"\n",
            ),
            (
                "m2.bash",
                "#!/bin/bash\nmain() { set -euo pipefail; cd /tmp; ls; }\nmain \"$@\"\n",
            ),
            (
                "m3.bash",
                "#!/bin/bash\nmain() {\n\n  set -e\n  set -u\n  set -o pipefail\n  echo hi\n}\nmain\n",
            ),
            (
                "m4.zsh",
                "#!/bin/zsh\nmain() {\n  setopt err_exit no_unset pipe_fail\n  echo hi\n}\nmain\n",
            ),
        ],
    );
    assert_hazard_rule(
        "unchecked-cd",
        &[
            (
                "a.bash",
                "#!/bin/bash\nmain() {\n  echo hi\n  cd /tmp\n}\nmain\n",
                &[4],
            ),
            (
                "b.bash",
                "#!/bin/bash\nmain() {\n  set -u\n  cd /tmp\n}\nmain\n",
                &[4],
            ),
            (
                "c.bash",
                "#!/bin/bash\nmain() {\n  echo hi\n  set -e\n  cd /tmp\n}\nmain\n",
                &[5],
            ),
            (
                "d.bash",
                "#!/bin/bash\nhelper() {\n  set -e\n  cd /a\n}\ncd /b\nhelper\n",
                &[6],
            ),
        ],
        &[
            (
                "m1.bash",
                "#!/bin/bash\nmain() {\n  set -euo pipefail\n  cd /tmp\n}\nmain \"$@\"\n",
            ),
            (
                "m2.bash",
                "#!/bin/bash\nhelper() {\n  set -e\n  cd /tmp\n}\n",
            ),
            (
                "m3.bash",
                "#!/bin/bash\nhelper() {\n  set -e\n  echo \"$(cd /tmp && pwd)\"\n}\n",
            ),
        ],
    );
}

#[test]
fn planted_defects_heredoc_delimiters_are_not_unquoted_variables() {
    assert_hazard_rule(
        "unquoted-variable",
        &[(
            "a.sh",
            "#!/bin/sh\nset -eu\ncat <<EOF\nvalue $x\nEOF\necho $y\n",
            &[6],
        )],
        &[
            ("m1.sh", "#!/bin/sh\nset -eu\ncat <<$EOF\nbody\n$EOF\n"),
            ("m2.sh", "#!/bin/sh\nset -eu\ncat <<-$END\nbody\n$END\n"),
        ],
    );
}
