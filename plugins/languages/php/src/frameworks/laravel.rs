use super::{ComposerPackage, ConventionRules};

const LOCAL_MARKERS: [&str; 2] = ["artisan", "bootstrap/app.php"];

pub(super) const RULES: ConventionRules = ConventionRules {
    core_entries: &[
        "public/index.php",
        "artisan",
        "bootstrap/app.php",
        "bootstrap/providers.php",
    ],
    direct_php_dirs: &["config", "database/migrations"],
    direct_suffixed_dirs: &[],
    nested_php_dirs: &[
        "routes",
        "app/Console/Commands",
        "app/Listeners",
        "app/Policies",
        "app/Models/Policies",
        "app/Providers",
        "database/seeders",
        "database/factories",
    ],
    nested_suffixed_dirs: &[],
    generated_files: &["public/hot"],
    generated_dirs: &["bootstrap/cache", "storage/framework", "public/build"],
    fallback_mappings: &[
        ("App\\", "app"),
        ("Database\\Factories\\", "database/factories"),
        ("Database\\Seeders\\", "database/seeders"),
    ],
};

pub(super) fn detect(pkg: &ComposerPackage<'_>) -> bool {
    pkg.requires("laravel/framework")
        && (pkg.contains_any_file(&LOCAL_MARKERS) || pkg.contains_php_under("routes"))
}
