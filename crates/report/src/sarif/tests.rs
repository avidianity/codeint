use super::relative_uri;

#[test]
fn a_path_of_letters_digits_dots_dashes_and_slashes_is_its_own_uri() {
    assert_eq!(
        relative_uri("packages/web-app/src/v2/index.test.ts"),
        "packages/web-app/src/v2/index.test.ts"
    );
}

#[test]
fn a_space_a_percent_sign_and_a_hash_are_percent_encoded() {
    assert_eq!(relative_uri("src/my file%#1.ts"), "src/my%20file%25%231.ts");
}

#[test]
fn a_colon_is_percent_encoded_so_the_first_segment_never_reads_as_a_uri_scheme() {
    assert_eq!(relative_uri("c:/notes.ts"), "c%3A/notes.ts");
}

#[test]
fn each_utf_8_byte_of_a_non_ascii_character_is_percent_encoded() {
    assert_eq!(relative_uri("src/café.ts"), "src/caf%C3%A9.ts");
}
