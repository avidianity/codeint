use super::{ComposerPackage, ConventionRules};

const COMPOSER_LOCAL_MARKERS: [&str; 4] = [
    "spark",
    "app/Config/Routes.php",
    "app/Config/App.php",
    "public/index.php",
];

pub(super) const RULES: ConventionRules = ConventionRules {
    core_entries: &["public/index.php", "spark"],
    direct_php_dirs: &["app/Config", "app/Config/Boot", "app/Database/Migrations"],
    direct_suffixed_dirs: &[("app/Helpers", "_helper.php")],
    nested_php_dirs: &[
        "app/Controllers",
        "app/Commands",
        "app/Filters",
        "app/Views",
        "app/Database/Seeds",
    ],
    nested_suffixed_dirs: &[],
    generated_files: &[],
    generated_dirs: &["writable"],
    fallback_mappings: &[("App\\", "app"), ("Config\\", "app/Config")],
};

pub(super) fn detect(pkg: &ComposerPackage<'_>) -> bool {
    let composer_install =
        pkg.requires("codeigniter4/framework") && pkg.contains_any_file(&COMPOSER_LOCAL_MARKERS);
    let manual_install = pkg.contains_file("spark")
        && pkg.contains_file("app/Config/Paths.php")
        && pkg.contains_file("system/Boot.php");
    composer_install || manual_install
}
