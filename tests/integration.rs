mod common;

oxic_test::oxic_test!();

#[path = "integration/attributes.rs"]
mod attributes;

#[path = "integration/expressions.rs"]
mod expressions;

#[path = "integration/functions.rs"]
mod functions;

#[path = "integration/interfaces.rs"]
mod interfaces;

#[path = "integration/modules.rs"]
mod modules;

#[path = "integration/structs.rs"]
mod structs;

#[path = "integration/unicode.rs"]
mod unicode;

#[path = "integration/variables.rs"]
mod variables;
