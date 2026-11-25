pub(super) fn escape_control_characters(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_debug().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}
