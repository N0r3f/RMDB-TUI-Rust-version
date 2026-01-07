pub fn truncate_string(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        format!("{}...", s.chars().take(max_chars.saturating_sub(3)).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn truncate_string_exact(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect::<String>()
}

