//! Messaging infrastructure — adapters that publish domain events to the message bus.
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`).

pub mod posting_event_sink;

pub use posting_event_sink::{
    AccountingPostFailedIntegration, AccountingPostPostedIntegration, MessagingSink,
};
