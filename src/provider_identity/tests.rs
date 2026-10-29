use super::*;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn accepted_ids_do_not_retain_unbounded_input_capacity() {
    let mut old = String::with_capacity(1024 * 1024);
    old.push_str("old");
    let mut new = String::with_capacity(1024 * 1024);
    new.push_str("new");
    let aliases = ProviderAliases::new(vec![(old, new)]).unwrap();
    for (old, new) in &aliases.0 {
        assert_eq!(old.capacity(), old.len());
        assert_eq!(new.capacity(), new.len());
    }
    assert_eq!(ProviderAliases::WAIT_SNAPSHOT_BYTES, 768 * 1024);
}

#[test]
fn expands_only_the_explicitly_authorized_identity_in_both_directions() {
    let aliases = ProviderAliases::new(pairs(&[("old", "new"), ("other", "other-api")])).unwrap();
    assert_eq!(aliases.expand(&strings(&["old"])), strings(&["old", "new"]));
    assert_eq!(aliases.expand(&strings(&["new"])), strings(&["new", "old"]));
    assert_eq!(
        aliases.expand(&strings(&["unrelated"])),
        strings(&["unrelated"])
    );
    assert!(aliases.expand(&[]).is_empty());
}

#[test]
fn deduplicates_without_changing_existing_order_or_case() {
    let aliases = ProviderAliases::new(pairs(&[("old", "xAI-api")])).unwrap();
    assert_eq!(
        aliases.expand(&strings(&["old", "old", "xAI-api"])),
        strings(&["old", "xAI-api"])
    );
    assert_eq!(
        aliases.expand(&strings(&["xai-api"])),
        strings(&["xai-api"])
    );
}

#[test]
fn rejects_ambiguous_merges_chains_cycles_and_invalid_identifiers() {
    for values in [
        vec![("a", "b"), ("b", "c")],
        vec![("a", "b"), ("b", "a")],
        vec![("a", "c"), ("b", "c")],
        vec![("a", "b"), ("a", "c")],
        vec![("a", "a")],
        vec![("", "b")],
        vec![("a", " b")],
        vec![("a\n", "b")],
    ] {
        assert!(ProviderAliases::new(pairs(&values)).is_err());
    }
    assert!(ProviderAliases::new(vec![("a".repeat(257), "b".into())]).is_err());
}

#[test]
fn limits_registry_size_and_expansion() {
    let pairs: Vec<_> = (0..256)
        .map(|i| (format!("old-{i}"), format!("new-{i}")))
        .collect();
    let aliases = ProviderAliases::new(pairs.clone()).unwrap();
    let allowed: Vec<_> = (0..128).map(|i| format!("old-{i}")).collect();
    assert_eq!(aliases.expand(&allowed).len(), 256);
    let mut too_many = pairs;
    too_many.push(("overflow".into(), "overflow-api".into()));
    assert!(ProviderAliases::new(too_many).is_err());
}

#[test]
fn selects_exactly_one_published_name_without_inferring_or_merging() {
    let aliases = ProviderAliases::new(pairs(&[("old", "new")])).unwrap();
    for (requested, present) in [("old", "new"), ("new", "old"), ("new", "new")] {
        assert_eq!(
            aliases.select_present(requested, |id| id == present),
            Ok(Some(present))
        );
    }
    assert!(aliases.select_present("old", |_| true).is_err());
    assert_eq!(aliases.select_present("old", |_| false), Ok(None));
    assert_eq!(aliases.select_present("OLD", |id| id == "new"), Ok(None));
    assert_eq!(
        aliases.select_present("other", |id| id == "other"),
        Ok(Some("other"))
    );
}
