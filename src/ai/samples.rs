//! Embedded scan evidence used to exercise identification without touching the network.

use serde_json::Value;

const SAMPLES: &str = include_str!("device-evidence-samples.json");

/// Pretty-printed evidence for the sample at `index`, without its UI-only sample name.
pub(crate) fn evidence_sample(index: usize) -> Option<String> {
    let document: Value = serde_json::from_str(SAMPLES).ok()?;
    let evidence = document.as_array()?.get(index)?.get("evidence")?;
    serde_json::to_string_pretty(evidence).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_named_samples_hold_json_evidence_arrays() {
        let document: Value = serde_json::from_str(SAMPLES).unwrap();
        let samples = document.as_array().unwrap();
        assert_eq!(samples.len(), 10);
        for sample in samples {
            assert!(sample.get("name").is_some_and(Value::is_string));
            assert!(sample.get("evidence").is_some_and(Value::is_array));
        }
    }

    #[test]
    fn every_sample_can_be_selected_and_the_index_is_bounded() {
        for index in 0..10 {
            assert!(evidence_sample(index).is_some());
        }
        assert!(evidence_sample(10).is_none());
    }
}
