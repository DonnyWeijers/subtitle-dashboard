//! Types shared by ingest (stored as JSON columns) and the API.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

/// Missing and `null` both become the default value.
pub fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// One issue the AI judge found in a translation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    pub line: Option<i64>,
    /// For example `meaning`, `untranslated`, `grammar`.
    #[serde(alias = "type", default)]
    #[schema(required = true)]
    pub kind: String,
    #[serde(default)]
    #[schema(required = true)]
    pub note: String,
}

/// Step 1's output when it failed a free check. subtrans then repairs it: first only the failing batches with
/// step 1's model (once per retry effort), then those batches with step 2's model, and only if the file still fails,
/// the whole file with step 2's model.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Step1 {
    pub model: Option<String>,
    /// Share of lines per check, 0–1.
    #[serde(default, deserialize_with = "null_default")]
    #[schema(required = true)]
    pub checks: BTreeMap<String, f64>,
    #[serde(default, deserialize_with = "null_default")]
    #[schema(required = true)]
    pub failed: Vec<String>,
    /// The repair passes, in order.
    #[serde(default, deserialize_with = "null_default")]
    #[schema(required = true)]
    pub redo: Vec<Redo>,
}

/// One repair pass.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Redo {
    pub model: Option<String>,
    /// Reasoning effort, for step 1's model.
    pub effort: Option<String>,
    /// How many 100-line batches were redone; `None` for the whole file.
    #[serde(default, deserialize_with = "batches")]
    pub batches: Option<i64>,
    /// Checks the file still failed after this pass.
    #[serde(default, deserialize_with = "null_default")]
    #[schema(required = true)]
    pub failed: Vec<String>,
}

/// subtrans writes a batch count, or `"all"` for the whole file.
fn batches<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(serde_json::Value::deserialize(d)?.as_i64())
}

impl Step1 {
    /// How step 1's failure was repaired. `final_failed` is what the finished translation still fails.
    pub fn repair(&self, final_failed: &[String]) -> Repair {
        if self.redo.is_empty() {
            // Before the batch repair (2026-10-06) a failed step 1 always went straight to step 2, whole file.
            return if final_failed.is_empty() { Repair::Step2File } else { Repair::Unfixed };
        }
        match self.redo.iter().find(|r| r.failed.is_empty()) {
            None => Repair::Unfixed,
            Some(r) if r.batches.is_none() => Repair::Step2File,
            Some(r) if r.model.is_some() && r.model == self.model => Repair::Step1Batches,
            Some(_) => Repair::Step2Batches,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Repair {
    /// Redoing the failing batches with step 1's model was enough.
    Step1Batches,
    /// The failing batches needed step 2's model.
    Step2Batches,
    /// The whole file was redone with step 2's model.
    Step2File,
    /// Still failing a check after every pass: a weak translation.
    Unfixed,
}

impl Repair {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Step1Batches => "step1-batches",
            Self::Step2Batches => "step2-batches",
            Self::Step2File => "step2-file",
            Self::Unfixed => "unfixed",
        }
    }
}

/// One attempt on a subtitle, oldest first.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Attempt {
    #[serde(alias = "tier")]
    pub step: Option<i64>,
    pub score: Option<f64>,
    #[serde(default, deserialize_with = "null_default")]
    #[schema(required = true)]
    pub failed: Vec<String>,
    pub at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step1(redo: serde_json::Value) -> Step1 {
        serde_json::from_value(serde_json::json!({"model": "luna", "failed": ["shifted"], "redo": redo})).unwrap()
    }

    #[test]
    fn repair_route() {
        use serde_json::json;
        let pass = |model: &str, batches: serde_json::Value, failed: &[&str]| json!({"model": model, "effort": null, "batches": batches, "failed": failed});
        let ok: &[String] = &[];
        let failing = &["missing".to_string()];
        assert_eq!(step1(json!([pass("luna", json!(2), &[])])).repair(ok), Repair::Step1Batches);
        assert_eq!(
            step1(json!([pass("luna", json!(2), &["shifted"]), pass("gemini", json!(1), &[])])).repair(ok),
            Repair::Step2Batches
        );
        assert_eq!(
            step1(json!([pass("gemini", json!(1), &["missing"]), pass("gemini", json!("all"), &[])])).repair(ok),
            Repair::Step2File
        );
        assert_eq!(step1(json!([pass("gemini", json!("all"), &["missing"])])).repair(failing), Repair::Unfixed);
        // Older entries have no passes: step 2 redid the whole file.
        assert_eq!(step1(json!(null)).repair(ok), Repair::Step2File);
        assert_eq!(step1(json!(null)).repair(failing), Repair::Unfixed);
    }
}
