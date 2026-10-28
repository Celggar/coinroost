//! The JSON form of a normalization, shared by the command line and the fixture tests.

use serde_json::{Value, json};

use crate::{Normalization, RowProblem};

impl Normalization {
    /// Transactions with the amount as written and in minor units, error rows, and the counts.
    pub fn to_json(&self) -> Value {
        let transactions: Vec<Value> = self
            .transactions
            .iter()
            .map(|t| {
                json!({
                    "date": t.date,
                    "amount": t.amount.to_string(),
                    "amount_minor": t.amount.minor_units(),
                    "description": t.description,
                    "external_id": t.external_id,
                })
            })
            .collect();
        let errors: Vec<Value> = self
            .errors
            .iter()
            .map(|e| {
                let problem = match &e.problem {
                    RowProblem::UnreadableDate(value) => format!("unreadable date: {value}"),
                    RowProblem::UnreadableAmount => "unreadable amount".to_owned(),
                };
                json!({ "row": e.row, "problem": problem })
            })
            .collect();
        json!({
            "fatal": null,
            "rows": self.summary.rows,
            "structural": self.summary.structural,
            "ambiguities": self.ambiguities.len(),
            "transactions": transactions,
            "errors": errors,
        })
    }
}
