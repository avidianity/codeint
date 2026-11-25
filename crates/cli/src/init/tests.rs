use super::roll_back;

#[test]
fn roll_back_removes_written_files_and_the_created_folder() {
    let root = tempfile::tempdir().expect("temporary folder");
    let folder = root.path().join(".codeint");
    std::fs::create_dir(&folder).expect("create the folder");
    let written = folder.join("ratchet.json");
    std::fs::write(&written, "{}").expect("write a file");

    let leftovers = roll_back(
        &[written.clone(), folder.join("baseline.json")],
        Some(folder.as_path()),
    );

    assert!(leftovers.is_empty(), "got: {leftovers:?}");
    assert!(!folder.exists());
}

#[test]
fn roll_back_reports_each_path_it_cannot_remove() {
    let root = tempfile::tempdir().expect("temporary folder");
    let stuck_file = root.path().join("codeint.jsonc");
    std::fs::create_dir_all(stuck_file.join("inner")).expect("make a non-empty folder");
    let stuck_folder = root.path().join(".codeint");
    std::fs::create_dir_all(stuck_folder.join("inner")).expect("make a non-empty folder");

    let leftovers = roll_back(
        std::slice::from_ref(&stuck_file),
        Some(stuck_folder.as_path()),
    );

    assert_eq!(leftovers, vec![stuck_file, stuck_folder]);
}
