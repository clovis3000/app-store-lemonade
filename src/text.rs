//! Display helpers that respect UTF-8 character boundaries.

/// Shortens `s` to at most `max` characters, ending in "…" when it cuts.
///
/// Slicing a `&str` by byte index panics mid-character, and app names and
/// review bodies are full of emoji, accents and currency symbols.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn leaves_short_strings_alone() {
        assert_eq!(truncate("Notes", 10), "Notes");
        assert_eq!(truncate("exactly10!", 10), "exactly10!");
    }

    #[test]
    fn cuts_on_character_boundaries() {
        // Byte 29 of this name falls inside "€", which used to panic.
        let name = "CalConvert: Pro Calculator $€ and more";
        let cut = truncate(name, 30);
        assert_eq!(cut.chars().count(), 30);
        assert!(cut.ends_with('…'));
        assert_eq!(truncate("😀😀😀😀", 3), "😀😀…");
    }
}
