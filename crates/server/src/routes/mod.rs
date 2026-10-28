//! Route handlers, one module per resource. Each module's `router()` registers its handlers together with their
//! entry in the API contract, so a route cannot exist without being in the contract.

pub mod accounts;
pub mod apply;
pub mod catalog;
pub mod coverage;
pub mod imports;
pub mod payroll;
pub mod reports;
pub mod rules;
pub mod session;
pub mod summary;
pub mod system;
pub mod transactions;
pub mod transfers;
