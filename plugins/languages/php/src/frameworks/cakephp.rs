use super::{ComposerPackage, ConventionRules};

const LOCAL_MARKERS: [&str; 5] = [
    "src/Application.php",
    "bin/cake",
    "bin/cake.php",
    "config/routes.php",
    "webroot/index.php",
];

pub(super) const RULES: ConventionRules = ConventionRules {
    core_entries: &[
        "webroot/index.php",
        "bin/cake",
        "bin/cake.php",
        "src/Application.php",
        "config/bootstrap.php",
        "config/bootstrap_cli.php",
        "config/routes.php",
        "config/paths.php",
        "config/app.php",
    ],
    direct_php_dirs: &["config/Migrations", "config/Seeds"],
    direct_suffixed_dirs: &[],
    nested_php_dirs: &["src/Model/Entity", "templates"],
    nested_suffixed_dirs: &[
        ("src/Controller", "Controller.php"),
        ("src/Command", "Command.php"),
        ("src/Model/Table", "Table.php"),
        ("src/View/Helper", "Helper.php"),
        ("src/View/Cell", "Cell.php"),
        ("src/Mailer", "Mailer.php"),
    ],
    generated_files: &[],
    generated_dirs: &["tmp", "logs"],
    fallback_mappings: &[("App\\", "src")],
};

pub(super) fn detect(pkg: &ComposerPackage<'_>) -> bool {
    pkg.requires("cakephp/cakephp")
        && pkg.package_type() != Some("cakephp-plugin")
        && pkg.contains_any_file(&LOCAL_MARKERS)
}
