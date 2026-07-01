//! Real message-bus adapter: publishes GL-posting domain events to the backbone-messaging
//! `IntegrationEventBus` (in-memory transport; redis/rabbitmq/kafka are future features).
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Implements the `PostingEventSink`
//! seam defined in `application::service::posting_service`, mapping the module's domain events
//! to **integration events** (string ids, dot-notation topics) for cross-module consumption.

use std::sync::Arc;

use backbone_messaging::{EventError, IntegrationEvent, IntegrationEventBus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::application::service::posting_service::{PostingEvent, PostingEventSink};

/// Cross-module event: a GL posting was recorded. Topic `accounting.posting.posted`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountingPostPostedIntegration {
    pub post_id: String,
    pub journal_id: String,
    pub company_id: String,
    pub source_type: String,
    pub source_id: String,
    pub total_debit: String,
    pub total_credit: String,
    pub occurred_at: DateTime<Utc>,
}

impl IntegrationEvent for AccountingPostPostedIntegration {
    fn event_type(&self) -> &'static str {
        "accounting.posting.posted"
    }
    fn source_context(&self) -> &'static str {
        "accounting"
    }
    fn aggregate_id(&self) -> &str {
        &self.post_id
    }
    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }
}

/// Cross-module event: a GL posting was rejected. Topic `accounting.posting.failed`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountingPostFailedIntegration {
    pub company_id: String,
    pub source_type: String,
    pub source_id: String,
    pub error_code: String,
    pub error_message: String,
    pub occurred_at: DateTime<Utc>,
}

impl IntegrationEvent for AccountingPostFailedIntegration {
    fn event_type(&self) -> &'static str {
        "accounting.posting.failed"
    }
    fn source_context(&self) -> &'static str {
        "accounting"
    }
    fn aggregate_id(&self) -> &str {
        &self.source_id
    }
    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }
}

/// `PostingEventSink` backed by the integration event bus.
#[derive(Clone)]
pub struct MessagingSink {
    bus: Arc<IntegrationEventBus>,
}

impl MessagingSink {
    pub fn new(bus: Arc<IntegrationEventBus>) -> Self {
        Self { bus }
    }

    /// Publish awaitably (deterministic — used by tests and by the fire-and-forget trait impl).
    pub async fn publish_async(&self, event: PostingEvent) -> Result<(), EventError> {
        match event {
            PostingEvent::AccountingPostPosted(e) => {
                self.bus
                    .publish(AccountingPostPostedIntegration {
                        post_id: e.post_id.to_string(),
                        journal_id: e.journal_id.to_string(),
                        company_id: e.company_id.to_string(),
                        source_type: e.source_type,
                        source_id: e.source_id.to_string(),
                        total_debit: e.total_debit.to_string(),
                        total_credit: e.total_credit.to_string(),
                        occurred_at: e.occurred_at,
                    })
                    .await
            }
            PostingEvent::AccountingPostFailed(e) => {
                self.bus
                    .publish(AccountingPostFailedIntegration {
                        company_id: e.company_id.to_string(),
                        source_type: e.source_type,
                        source_id: e.source_id.to_string(),
                        error_code: e.error_code,
                        error_message: e.error_message,
                        occurred_at: e.occurred_at,
                    })
                    .await
            }
        }
    }
}

impl PostingEventSink for MessagingSink {
    /// Fire-and-forget: the domain post is already committed, so bus delivery must not block or
    /// fail the posting. Spawns onto the ambient Tokio runtime.
    fn publish(&self, event: PostingEvent) {
        let sink = self.clone();
        tokio::spawn(async move {
            if let Err(e) = sink.publish_async(event).await {
                tracing::warn!(target: "accounting.events", error = %e, "failed to publish posting event to bus");
            }
        });
    }
}
