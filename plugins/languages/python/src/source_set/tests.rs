use codeint_model::{LanguagePlugin, RepoPath};

use super::is_tooling_file;
use crate::Python;

#[test]
fn classifies_python_tooling_files_and_preserves_nested_runtime_modules() {
    for path in [
        "conftest.py",
        "tests/conftest.py",
        "src/tests/conftest.py",
        "conf.py",
        "setup.py",
        "noxfile.py",
        "fabfile.py",
        "packages/api/conf.py",
        "packages/api/setup.py",
        "packages/app/setup.py",
        "packages/py/noxfile.py",
        "packages/py/fabfile.py",
    ] {
        assert!(is_tooling_file(path, &[]), "expected tooling file: {path}");
    }
    assert!(is_tooling_file("backend/conf.py", &["sphinx".to_owned()]));
    assert!(is_tooling_file(
        "backend/setup.py",
        &["setuptools".to_owned()]
    ));
    assert!(is_tooling_file("backend/noxfile.py", &["nox".to_owned()]));
    assert!(is_tooling_file(
        "backend/fabfile.py",
        &["fabric".to_owned()]
    ));

    for path in [
        "myapp/conf.py",
        "django_app/core/conf.py",
        "myapp/billing/setup.py",
        "packages/api/src/conf.py",
        "packages/api/src/setup.py",
        "src/conf.py",
        "src/setup.py",
        "src/noxfile.py",
        "src/fabfile.py",
        "src/app.py",
    ] {
        assert!(
            !is_tooling_file(path, &[]),
            "unexpected tooling file: {path}"
        );
    }
}

#[test]
fn lower_sets_is_tooling_file_for_python_tooling_files() {
    let lowered_setup = Python
        .lower(
            &RepoPath::new("packages/py/setup.py").unwrap(),
            "from setuptools import setup\nsetup(name='demo')\n",
            None,
        )
        .unwrap();
    let lowered_runtime = Python
        .lower(
            &RepoPath::new("myapp/billing/setup.py").unwrap(),
            "def init_billing():\n    return True\n",
            None,
        )
        .unwrap();
    assert!(lowered_setup.is_tooling_file && !lowered_runtime.is_tooling_file);
}
