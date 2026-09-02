use serde::Deserialize;
use serde_json::Value;

use std::collections::{BTreeMap, HashMap};

#[derive(Deserialize)]
pub struct ProofFile<'a> {
    pub profile: Value,
    #[serde(borrow)]
    pub proofs: HashMap<&'a str, BTreeMap<&'a str, ProofCertificate<'a>>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ProofCertificate<'a> {
    Stuck,
    Proven {
        prover: &'a str,
        time: f64,
    },
    Tactic {
        tactic: &'a str,
        children: Vec<ProofCertificate<'a>>,
    },
}

impl<'a> ProofCertificate<'a> {
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Stuck => false,
            Self::Proven { .. } => true,
            Self::Tactic {
                tactic: _,
                children,
            } => children.iter().all(Self::is_complete),
        }
    }
}
