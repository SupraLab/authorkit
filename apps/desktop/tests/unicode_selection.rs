//! Selection / wrap behaviour with accents and apostrophes.

fn body_char_slice(body: &str, start: usize, end: usize) -> String {
    body.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn replace_range(body: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut chars: Vec<char> = body.chars().collect();
    chars.splice(start..end, replacement.chars());
    chars.into_iter().collect()
}

#[test]
fn accented_selection_replace_keeps_neighbours() {
    let body = "L'été était brûlant, dit-elle.";
    // Select "était" (chars after "L'été "); find is byte index — convert carefully.
    let start = body.find("était").unwrap();
    let start_char = body[..start].chars().count();
    let end_char = start_char + "était".chars().count();
    assert_eq!(body_char_slice(body, start_char, end_char), "était");
    let out = replace_range(body, start_char, end_char, "fut");
    assert_eq!(out, "L'été fut brûlant, dit-elle.");
}

#[test]
fn wrap_bold_around_apostrophe_phrase() {
    let body = "C'est l'heure.";
    // select "C'est"
    let start = 0usize;
    let end = "C'est".chars().count();
    let sel = body_char_slice(body, start, end);
    assert_eq!(sel, "C'est");
    let wrapped = format!("**{sel}**");
    let out = replace_range(body, start, end, &wrapped);
    assert_eq!(out, "**C'est** l'heure.");
    // Re-select inner
    let inner_start = 2; // after **
    let inner_end = inner_start + sel.chars().count();
    assert_eq!(body_char_slice(&out, inner_start, inner_end), "C'est");
}

#[test]
fn successive_wraps_do_not_drift_with_unicode() {
    let mut body = "Naïve façade — « déjà ».".to_string();
    // Bold whole string
    let n = body.chars().count();
    body = replace_range(&body, 0, n, &format!("**{body}**"));
    // Italic inside (skip **)
    let inner_start = 2;
    let inner_end = body.chars().count() - 2;
    let inner = body_char_slice(&body, inner_start, inner_end);
    body = replace_range(&body, inner_start, inner_end, &format!("*{inner}*"));
    assert!(body.starts_with("***") || body.starts_with("**"));
    assert!(body.contains("Naïve"));
    assert!(body.contains("façade"));
    assert!(body.contains("déjà"));
}
