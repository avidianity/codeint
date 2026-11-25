use super::{ComposerPackage, ConventionRules};

const LOCAL_MARKERS: [&str; 5] = [
    "config/bundles.php",
    "src/Kernel.php",
    "bin/console",
    "public/index.php",
    "symfony.lock",
];

pub(super) const RULES: ConventionRules = ConventionRules {
    core_entries: &[
        "public/index.php",
        "bin/console",
        "src/Kernel.php",
        "config/bundles.php",
        "config/preload.php",
        "config/routes.php",
        "config/services.php",
    ],
    direct_php_dirs: &["migrations"],
    direct_suffixed_dirs: &[],
    nested_php_dirs: &[
        "src/Controller",
        "src/Command",
        "src/EventListener",
        "src/EventSubscriber",
        "src/Twig",
        "src/Security",
        "src/Form",
        "src/DataFixtures",
    ],
    nested_suffixed_dirs: &[],
    generated_files: &[],
    generated_dirs: &["var/cache", "var/log", "public/bundles", "public/build"],
    fallback_mappings: &[("App\\", "src"), ("DoctrineMigrations\\", "migrations")],
};

pub(super) fn detect(pkg: &ComposerPackage<'_>) -> bool {
    let has_framework_bundle = pkg.requires("symfony/framework-bundle")
        || pkg.requires_in("flex-require", "symfony/framework-bundle");
    has_framework_bundle
        && pkg.package_type() != Some("symfony-bundle")
        && pkg.contains_any_file(&LOCAL_MARKERS)
}
