use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

pub(crate) fn filter_handles_fuzzy(handles: &[String], current: &str) -> Vec<String> {
    if current.trim().is_empty() {
        return handles.to_vec();
    }

    let matcher = matcher();
    let source = normalize(current);
    let typed_handle = current.contains("://");

    let mut ranked: Vec<(i64, String)> = handles
        .iter()
        .filter_map(|handle| {
            let candidate = if typed_handle {
                handle.as_str()
            } else {
                handle
                    .split_once("://")
                    .map(|(_, slug)| slug)
                    .unwrap_or(handle)
            };

            matcher
                .fuzzy_match(&normalize(candidate), &source)
                .map(|score| (score, handle.clone()))
        })
        .collect();

    ranked.sort_by(|(score_a, value_a), (score_b, value_b)| {
        score_b.cmp(score_a).then_with(|| value_a.cmp(value_b))
    });
    ranked.into_iter().map(|(_, value)| value).collect()
}

pub(crate) fn filter_slugs_fuzzy(handles: &[String], current: &str) -> Vec<String> {
    let candidates: Vec<String> = handles
        .iter()
        .filter_map(|handle| handle.split_once("://").map(|(_, slug)| slug.to_string()))
        .collect();
    fuzzy_rank_strings(candidates, current)
}

pub(crate) fn fuzzy_rank_strings(candidates: Vec<String>, current: &str) -> Vec<String> {
    if current.trim().is_empty() {
        return candidates;
    }

    let matcher = matcher();
    let source = normalize(current);
    let mut ranked: Vec<(i64, String)> = candidates
        .into_iter()
        .filter_map(|candidate| {
            matcher
                .fuzzy_match(&normalize(&candidate), &source)
                .map(|score| (score, candidate))
        })
        .collect();

    ranked.sort_by(|(score_a, value_a), (score_b, value_b)| {
        score_b.cmp(score_a).then_with(|| value_a.cmp(value_b))
    });
    ranked.into_iter().map(|(_, value)| value).collect()
}

fn matcher() -> SkimMatcherV2 {
    SkimMatcherV2::default()
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}
