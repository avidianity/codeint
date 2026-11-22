use codeint_model::language::LowerError;

const MAX_NESTING_DEPTH: usize = 256;

pub(crate) fn check_nesting_depth(text: &str) -> Result<(), LowerError> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut bracket_depth: usize = 0;
    let mut ternary_depth: usize = 0;
    let mut template_brace_depths: Vec<usize> = Vec::new();

    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                index = skip_quoted(bytes, index);
            }
            b'`' => {
                index = skip_template(
                    bytes,
                    index + 1,
                    &mut template_brace_depths,
                    &mut bracket_depth,
                )?;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = skip_line_comment(bytes, index + 2);
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index + 2);
            }
            b'{' | b'(' | b'[' => {
                bracket_depth += 1;
                if bracket_depth > MAX_NESTING_DEPTH {
                    return Err(exceeded());
                }
                index += 1;
            }
            b'}' => {
                if template_brace_depths.last() == Some(&bracket_depth) {
                    template_brace_depths.pop();
                    bracket_depth = bracket_depth.saturating_sub(1);
                    ternary_depth = 0;
                    index = skip_template(
                        bytes,
                        index + 1,
                        &mut template_brace_depths,
                        &mut bracket_depth,
                    )?;
                } else {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    ternary_depth = 0;
                    index += 1;
                }
            }
            b')' | b']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                index += 1;
            }
            b'?' => {
                if is_ternary_question(bytes, index) {
                    ternary_depth += 1;
                    if ternary_depth > MAX_NESTING_DEPTH {
                        return Err(exceeded());
                    }
                }
                index += 1;
            }
            b':' => {
                ternary_depth = ternary_depth.saturating_sub(1);
                index += 1;
            }
            b';' => {
                ternary_depth = 0;
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(())
}

fn exceeded() -> LowerError {
    LowerError::Unparsable(format!(
        "nesting depth exceeds the parser limit of {MAX_NESTING_DEPTH}"
    ))
}

fn is_ternary_question(bytes: &[u8], index: usize) -> bool {
    let previous = index.checked_sub(1).and_then(|pos| bytes.get(pos)).copied();
    let next = bytes.get(index + 1).copied();
    !matches!(previous, Some(b'?'))
        && !matches!(next, Some(b'?' | b'.' | b':' | b',' | b')' | b']' | b';'))
}

fn skip_quoted(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = (index + 2).min(bytes.len()),
            byte if byte == quote => return index + 1,
            b'\n' | b'\r' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

fn skip_template(
    bytes: &[u8],
    mut index: usize,
    template_brace_depths: &mut Vec<usize>,
    bracket_depth: &mut usize,
) -> Result<usize, LowerError> {
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = (index + 2).min(bytes.len()),
            b'`' => return Ok(index + 1),
            b'$' if bytes.get(index + 1) == Some(&b'{') => {
                *bracket_depth += 1;
                if *bracket_depth > MAX_NESTING_DEPTH {
                    return Err(exceeded());
                }
                template_brace_depths.push(*bracket_depth);
                return Ok(index + 2);
            }
            _ => index += 1,
        }
    }
    Ok(bytes.len())
}

fn skip_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}
