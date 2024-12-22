/*
 * Utilities
 */

/// Indents every line of a string s by n spaces
pub fn indent(s: String, n: usize) -> String {
    let spaces = " ".repeat(n);
    let replacement = format!("\n{spaces}");
    s.replace("\n", replacement.as_str())
}

#[cfg(test)]
mod tests {
    use crate::utils::indent;

    #[test]
    fn test_indent() {
        let input_str = "hello\nworld\n  how's going?\n\nGreat, I guess...\n";
        let result = indent(input_str.into(), 4);
        let expected = "hello\n    world\n      how's going?\n    \n    Great, I guess...\n    ";
        assert_eq!(result, expected);
    }
}