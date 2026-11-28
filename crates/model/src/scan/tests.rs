use std::collections::HashSet;

use super::{RelativeFileResolution, resolve_relative_file};

#[test]
fn resolve_relative_file_never_returns_empty_external_sentinel() {
    let files: HashSet<String> = ["src/app.php", "src/helper.php", "vendor/autoload.php"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    assert_eq!(
        resolve_relative_file(&files, "src/app.php", "../vendor/autoload.php", true),
        RelativeFileResolution::Ignored
    );
    assert_eq!(
        resolve_relative_file(&files, "src/app.php", "./app.php", false),
        RelativeFileResolution::Ignored
    );
    assert_eq!(
        resolve_relative_file(&files, "src/app.php", "./helper.php", false),
        RelativeFileResolution::Resolved("src/helper.php".to_owned())
    );
    assert!(matches!(
        resolve_relative_file(&files, "src/app.php", "./missing.php", false),
        RelativeFileResolution::Unresolved(_)
    ));
    assert!(matches!(
        resolve_relative_file(&files, "src/app.php", "/etc/passwd", false),
        RelativeFileResolution::Unresolved(_)
    ));
    assert!(matches!(
        resolve_relative_file(&files, "src/app.php", "../../outside.php", false),
        RelativeFileResolution::Unresolved(_)
    ));
}
