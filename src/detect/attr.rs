//! Attribution: which layer(s) failed, finest cause, location class.

use std::collections::BTreeSet;

use crate::config::ProbeType;

/// Attribution class from the set of failing layers (roadmap §5.2).
pub fn attribution_class(failing: &BTreeSet<ProbeType>) -> &'static str {
    let has_dns = failing.contains(&ProbeType::Dns);
    let has_tcp = failing.contains(&ProbeType::Tcp);
    let has_http = failing.contains(&ProbeType::Http);
    if !has_dns && !has_tcp && !has_http {
        return "unknown";
    }
    if has_dns && has_tcp && has_http {
        return "link";
    }
    if has_dns && !has_tcp && !has_http {
        return "dns";
    }
    if has_http && !has_tcp {
        return "http/edge";
    }
    "partial"
}

/// Attribution class from a stored layers string ("dns;tcp").
pub fn attribution_class_str(layers: &str) -> &'static str {
    let set: BTreeSet<ProbeType> = layers
        .split(';')
        .filter_map(|s| match s {
            "dns" => Some(ProbeType::Dns),
            "tcp" => Some(ProbeType::Tcp),
            "http" => Some(ProbeType::Http),
            "icmp" => Some(ProbeType::Icmp),
            _ => None,
        })
        .collect();
    attribution_class(&set)
}

/// Finest cause: the most frequent detail sample, prefixed with its probe
/// type, e.g. "dns:query-timeout@1.1.1.1".
pub fn finest_cause(samples: &[(ProbeType, String)]) -> Option<String> {
    if samples.is_empty() {
        return None;
    }
    let mut counts: std::collections::HashMap<(ProbeType, String), usize> =
        std::collections::HashMap::new();
    for (t, d) in samples {
        *counts.entry((*t, d.clone())).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|((t, d), _)| format!("{t}:{d}"))
}

/// Semicolon-joined failing layers in canonical order, e.g. "dns;tcp".
pub fn layers_str(failing: &BTreeSet<ProbeType>) -> String {
    failing
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(";")
}

/// WiFi-vs-upstream from gateway probe counts during the window (roadmap §5.3).
pub fn location_class(gateway_ok: u32, gateway_fail: u32) -> String {
    match (gateway_ok > 0, gateway_fail > 0) {
        (false, false) => "unknown".into(),
        (false, true) => "local".into(),
        (true, false) => "upstream".into(),
        (true, true) => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(types: &[ProbeType]) -> BTreeSet<ProbeType> {
        types.iter().copied().collect()
    }

    #[test]
    fn attribution_classes() {
        assert_eq!(
            attribution_class(&set(&[ProbeType::Dns, ProbeType::Tcp, ProbeType::Http])),
            "link"
        );
        assert_eq!(attribution_class(&set(&[ProbeType::Dns])), "dns");
        assert_eq!(attribution_class(&set(&[ProbeType::Http])), "http/edge");
        assert_eq!(
            attribution_class(&set(&[ProbeType::Dns, ProbeType::Tcp])),
            "partial"
        );
        assert_eq!(attribution_class(&set(&[])), "unknown");
        // icmp-only failure: no external layer → unknown
        assert_eq!(attribution_class(&set(&[ProbeType::Icmp])), "unknown");
    }

    #[test]
    fn finest_cause_picks_most_frequent() {
        let samples = vec![
            (ProbeType::Dns, "query-timeout@1.1.1.1".into()),
            (ProbeType::Dns, "query-timeout@1.1.1.1".into()),
            (ProbeType::Dns, "no-answers@1.1.1.1".into()),
        ];
        assert_eq!(
            finest_cause(&samples).as_deref(),
            Some("dns:query-timeout@1.1.1.1")
        );
        assert_eq!(finest_cause(&[]), None);
    }

    #[test]
    fn location_class_matrix() {
        assert_eq!(location_class(0, 0), "unknown");
        assert_eq!(location_class(0, 3), "local");
        assert_eq!(location_class(5, 0), "upstream");
        assert_eq!(location_class(2, 1), "unknown");
    }

    #[test]
    fn layers_canonical_order() {
        assert_eq!(
            layers_str(&set(&[ProbeType::Http, ProbeType::Dns])),
            "dns;http"
        );
    }
}
