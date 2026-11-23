use codeint_model::language::{DepKind, Manifest};

use super::{extract_declared, extract_pep508_name, normalize_pep503};

#[test]
fn pep503_normalizes_separators_and_case() {
    for (raw, expected) in [
        ("Pydantic_Settings", "pydantic-settings"),
        ("zope.interface", "zope-interface"),
        ("My--Pkg__Name..V2", "my-pkg-name-v2"),
        ("Flask", "flask"),
    ] {
        assert_eq!(normalize_pep503(raw), expected);
    }
}

#[test]
fn pep508_extracts_package_names_from_specifiers() {
    assert_eq!(
        extract_pep508_name("uvicorn[standard]>=0.30 ; python_version >= '3.10'"),
        Some("uvicorn".into())
    );
    assert_eq!(
        extract_pep508_name("my_pkg @ https://example.invalid/my_pkg.whl"),
        Some("my-pkg".into())
    );
    assert_eq!(extract_pep508_name("   "), None);
    assert_eq!(extract_pep508_name(">=1.0"), None);
}

#[test]
fn parses_uv_pdm_poetry_and_skips_invalid_or_vendored_manifests() {
    let input = [
        (
            "pyproject.toml",
            "[tool.uv]\ndev-dependencies = [\"pytest_cov>=5.0\"]\n[tool.pdm.dev-dependencies]\nlint = [\"black>=24.0\"]\n",
        ),
        ("packages/broken/pyproject.toml", "[project\nbroken = true\n"),
        (
            ".venv/lib/pyproject.toml",
            "[project]\ndependencies = [\"ignored>=1.0\"]\n",
        ),
    ]
    .map(|(path, text)| Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    });
    let declared = extract_declared(&input);
    assert_eq!(declared.len(), 2);
    assert!(
        declared
            .iter()
            .all(|d| d.package_root.is_empty() && d.kind == DepKind::Dev)
    );
    assert_eq!(declared[0].name, "black");
    assert_eq!(declared[1].name, "pytest-cov");
}
