//! Coverage accounting for Flux. Discovery and assertion probes come next.
//! Evidence must belong to the same source, verifier, and configuration.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// The diagnostic specifically identified the injected assertion.
    Checked,
    /// Exemptions do not prove panic freedom.
    Trusted {
        reason: String,
    },
    Ignored {
        reason: String,
    },
    Silent,
    Unknown,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Include every function body and closure as a separate entry.
    pub inventory: BTreeSet<String>,
    pub inventory_complete: bool,
    pub evidence: BTreeMap<String, Evidence>,
    pub pending_ices: Vec<String>,
}

impl Report {
    /// Empty means the coverage gate passes; this is not a no-panic proof.
    pub fn blockers(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.inventory_complete {
            errors.push("body inventory is incomplete".into());
        }
        if self.inventory.is_empty() {
            errors.push("body inventory is empty".into());
        }
        for body in self.evidence.keys() {
            if !self.inventory.contains(body) {
                errors.push(format!("{body}: evidence has no inventory entry"));
            }
        }
        for body in &self.inventory {
            match self.evidence.get(body) {
                Some(Evidence::Checked) => {}
                Some(Evidence::Trusted { reason } | Evidence::Ignored { reason }) => {
                    if reason.trim().is_empty() {
                        errors.push(format!("{body}: exemption requires a reason"));
                    }
                }
                Some(Evidence::Silent) => errors.push(format!("{body}: probe is silent")),
                Some(Evidence::Unknown) => errors.push(format!("{body}: probe is unknown")),
                None => errors.push(format!("{body}: missing probe or exemption")),
            }
        }
        errors.extend(
            self.pending_ices
                .iter()
                .map(|ice| format!("unresolved ICE: {ice}")),
        );
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked() -> Report {
        Report {
            inventory: ["fn", "fn::closure#0"].map(String::from).into(),
            inventory_complete: true,
            evidence: ["fn", "fn::closure#0"]
                .map(|body| (body.into(), Evidence::Checked))
                .into(),
            pending_ices: vec![],
        }
    }

    #[test]
    fn every_body_and_closure_checked() {
        assert!(checked().blockers().is_empty());
    }

    #[test]
    fn explicit_exemptions_pass() {
        let mut report = checked();
        report.evidence.insert(
            "fn".into(),
            Evidence::Trusted {
                reason: "external contract".into(),
            },
        );
        report.evidence.insert(
            "fn::closure#0".into(),
            Evidence::Ignored {
                reason: "out of scope".into(),
            },
        );
        assert!(report.blockers().is_empty());
    }

    #[test]
    fn missing_closure_blocks() {
        let mut report = checked();
        report.evidence.remove("fn::closure#0");
        assert_eq!(
            report.blockers(),
            ["fn::closure#0: missing probe or exemption"]
        );
    }

    #[test]
    fn silent_and_interrupted_probes_block() {
        for evidence in [Evidence::Silent, Evidence::Unknown] {
            let mut report = checked();
            report.evidence.insert("fn::closure#0".into(), evidence);
            assert_eq!(report.blockers().len(), 1);
        }
    }

    #[test]
    fn exemptions_require_reasons() {
        for evidence in [
            Evidence::Trusted {
                reason: "  ".into(),
            },
            Evidence::Ignored {
                reason: String::new(),
            },
        ] {
            let mut report = checked();
            report.evidence.insert("fn".into(), evidence);
            assert_eq!(report.blockers(), ["fn: exemption requires a reason"]);
        }
    }

    #[test]
    fn unresolved_ice_blocks() {
        let mut report = checked();
        report.pending_ices.push("ice-1".into());
        assert_eq!(report.blockers(), ["unresolved ICE: ice-1"]);
    }

    #[test]
    fn empty_inventory_blocks() {
        let report = Report {
            inventory_complete: true,
            ..Report::default()
        };
        assert_eq!(report.blockers(), ["body inventory is empty"]);
    }

    #[test]
    fn incomplete_inventory_blocks() {
        let mut report = checked();
        report.inventory_complete = false;
        assert_eq!(report.blockers(), ["body inventory is incomplete"]);
    }

    #[test]
    fn unknown_body_evidence_blocks() {
        let mut report = checked();
        report.evidence.insert("stale_fn".into(), Evidence::Checked);
        assert_eq!(
            report.blockers(),
            ["stale_fn: evidence has no inventory entry"]
        );
    }
}
