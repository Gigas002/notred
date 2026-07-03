use super::util::truncate;

#[test]
fn truncate_short_unchanged() {
    assert_eq!(truncate("hi", 10), "hi");
}

#[test]
fn truncate_long_adds_ellipsis() {
    assert_eq!(truncate("hello world", 8), "hello w…");
}
