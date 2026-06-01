# Requirements Document

## Introduction

The Event-Driven Architecture (EDA) system provides a reliable, broker-agnostic asynchronous messaging layer for the Aframp platform. It decouples synchronous payment corridors (cNGN, mobile money, Stellar ledger settlement) from cross-cutting workflows — compliance checks, partner notifications, webhook dispatch, and ledger entry queuing — by routing domain events through a message broker abstraction backed by RabbitMQ or Apache Kafka. A transactional outbox pattern guarantees at-least-once delivery semantics without violating ACID properties on the primary database. The system is implemented in Rust and integrates with the existing Axum/SQLx/Tokio stack.

## Glossary

- **EDA_System**: The event-driven architecture subsystem of the Aframp backend.
- **Message_Broker**: The underlying message transport — either RabbitMQ or Apache Kafka — selected at deployment time via configuration.
- **Broker_Adapter**: The pluggable Rust trait implementation that abstracts over a specific Message_Broker, exposing a uniform publish/consume interface.
- **Domain_Event**: A structured, immutable record describing something that has happened in the platform (e.g. `PaymentInitiated`, `ComplianceCheckRequested`, `LedgerEntryQueued`).
- **Event_Envelope**: The wire format wrapping a Domain_Event — contains event type, event ID, aggregate ID, schema version, timestamp, correlation ID, and the serialised payload.
- **Outbox_Table**: The append-only database table used by the Transactional_Outbox to stage Domain_Events atomically with the originating database write.
- **Outbox_Relay**: The background worker that polls the Outbox_Table and publishes staged Domain_Events to the Message_Broker, then marks them as dispatched.
- **Transactional_Outbox**: The pattern combining the Outbox_Table and Outbox_Relay to guarantee at-least-once delivery without two-phase commit.
- **Consumer_Worker**: A background Tokio task that subscribes to a topic or queue on the Message_Broker and processes incoming Domain_Events.
- **Dead_Letter_Queue**: A dedicated queue or topic that receives Domain_Events that have exceeded the maximum retry count without successful processing.
- **Retry_Policy**: The configurable per-event-type backoff strategy — initial delay, multiplier, maximum delay, and maximum attempt count.
- **Schema_Registry**: The component that stores and validates Event_Envelope schemas, enforcing backward-compatible evolution.
- **Compliance_Worker**: The Consumer_Worker responsible for executing AML/KYC compliance checks triggered by Domain_Events.
- **Notification_Worker**: The Consumer_Worker responsible for dispatching partner and user notifications triggered by Domain_Events.
- **Webhook_Worker**: The Consumer_Worker responsible for delivering outbound webhook payloads to registered partner endpoints.
- **Ledger_Worker**: The Consumer_Worker responsible for queuing ledger entries on the Stellar network triggered by Domain_Events.
- **Idempotency_Key**: A unique identifier stored per processed Domain_Event to prevent duplicate side effects under at-least-once delivery.
- **Correlation_ID**: A propagated identifier linking a chain of Domain_Events back to the originating request for distributed tracing.
- **Test_Suite**: The collection of unit and integration tests for the EDA_System.

---

## Requirements

### Requirement 1: Broker Abstraction Layer

**User Story:** As a platform engineer, I want a broker-agnostic abstraction over RabbitMQ and Kafka, so that the platform can switch or support multiple brokers without changing business logic.

#### Acceptance Criteria

1. THE EDA_System SHALL define a `MessageBroker` Rust trait exposing `publish(envelope: EventEnvelope) -> Result<()>` and `subscribe(topic: &str, handler: ConsumerHandler) -> Result<()>` methods.
2. THE EDA_System SHALL provide a `RabbitMqAdapter` that implements the `MessageBroker` trait using AMQP 0-9-1 protocol.
3. THE EDA_System SHALL provide a `KafkaAdapter` that implements the `MessageBroker` trait using the Kafka producer/consumer API.
4. WHEN the broker type is set to `rabbitmq` in configuration, THE EDA_System SHALL instantiate the `RabbitMqAdapter` as the active `Broker_Adapter`.
5. WHEN the broker type is set to `kafka` in configuration, THE EDA_System SHALL instantiate the `KafkaAdapter` as the active `Broker_Adapter`.
6. IF the `Broker_Adapter` fails to connect at startup, THEN THE EDA_System SHALL retry connection using exponential backoff and SHALL emit a structured log event for each failed attempt.
7. THE EDA_System SHALL expose a health check endpoint that reports the connectivity status of the active `Broker_Adapter`.

---

### Requirement 2: Event Envelope and Schema

**User Story:** As a developer, I want every domain event to carry a standard envelope with versioned schema, so that consumers can deserialise events reliably and schema evolution is controlled.

#### Acceptance Criteria

1. THE EDA_System SHALL represent every Domain_Event on the wire as an Event_Envelope containing: `event_id` (UUID v4), `event_type` (string), `aggregate_id` (string), `schema_version` (semver string), `occurred_at` (ISO 8601 UTC timestamp), `correlation_id` (UUID v4), and `payload` (JSON object).
2. THE EDA_System SHALL serialise Event_Envelopes to JSON for transport over both RabbitMQ and Kafka.
3. THE Schema_Registry SHALL validate every outbound Event_Envelope against the registered schema for its `event_type` and `schema_version` before publishing.
4. IF an outbound Event_Envelope fails schema validation, THEN THE EDA_System SHALL reject the publish call with a typed error and SHALL NOT write the event to the Outbox_Table.
5. THE Schema_Registry SHALL enforce backward-compatible schema evolution: adding optional fields is permitted; removing fields or changing field types SHALL be rejected.
6. FOR ALL valid Event_Envelopes, serialising then deserialising SHALL produce an equivalent Event_Envelope (round-trip property).

---

### Requirement 3: Transactional Outbox Pattern

**User Story:** As a platform engineer, I want domain events to be written atomically with their originating database transaction, so that no event is lost if the broker is unavailable at the time of the business operation.

#### Acceptance Criteria

1. THE EDA_System SHALL provide an `OutboxWriter` that inserts a serialised Event_Envelope into the Outbox_Table within the same database transaction as the originating business write.
2. WHEN a business transaction commits, THE EDA_System SHALL guarantee that the corresponding Event_Envelope row exists in the Outbox_Table.
3. WHEN a business transaction rolls back, THE EDA_System SHALL guarantee that no Event_Envelope row is inserted into the Outbox_Table for that transaction.
4. THE Outbox_Table SHALL include columns: `id` (UUID), `event_type`, `aggregate_id`, `payload` (JSONB), `schema_version`, `correlation_id`, `created_at`, `dispatched_at` (nullable), `dispatch_attempts` (integer), and `status` (`pending` | `dispatched` | `failed`).
5. THE Outbox_Relay SHALL poll the Outbox_Table at a configurable interval, select rows with `status = 'pending'` ordered by `created_at` ascending, and publish each Event_Envelope to the Message_Broker.
6. WHEN the Outbox_Relay successfully publishes an Event_Envelope, THE EDA_System SHALL update the corresponding Outbox_Table row to `status = 'dispatched'` and set `dispatched_at` to the current UTC timestamp.
7. IF the Outbox_Relay fails to publish an Event_Envelope after exhausting the configured Retry_Policy, THEN THE EDA_System SHALL update the row to `status = 'failed'` and SHALL emit a structured log event containing the event ID, event type, aggregate ID, and final error.
8. THE Outbox_Relay SHALL process Outbox_Table rows in batches of a configurable size to bound memory usage.

---

### Requirement 4: At-Least-Once Delivery and Idempotency

**User Story:** As a platform engineer, I want consumer workers to process each event at least once and handle duplicates safely, so that retries do not cause double-charges, duplicate ledger entries, or repeated notifications.

#### Acceptance Criteria

1. THE EDA_System SHALL guarantee at-least-once delivery: every Domain_Event written to the Outbox_Table SHALL eventually be delivered to at least one Consumer_Worker unless it reaches `status = 'failed'`.
2. THE EDA_System SHALL provide an `IdempotencyStore` backed by the existing Redis cache that records processed Idempotency_Keys with a configurable TTL.
3. WHEN a Consumer_Worker receives a Domain_Event, THE EDA_System SHALL check the `IdempotencyStore` for the event's `event_id` before processing.
4. WHEN the `event_id` is already present in the `IdempotencyStore`, THE Consumer_Worker SHALL acknowledge the message without re-executing the handler and SHALL emit a structured log event indicating a duplicate was skipped.
5. WHEN the `event_id` is not present in the `IdempotencyStore`, THE Consumer_Worker SHALL execute the handler, then atomically record the `event_id` in the `IdempotencyStore` and acknowledge the message.
6. IF a Consumer_Worker handler returns an error, THEN THE EDA_System SHALL NOT record the `event_id` in the `IdempotencyStore` and SHALL allow the message to be redelivered according to the Retry_Policy.

---

### Requirement 5: Retry Policy and Dead Letter Queue

**User Story:** As a platform engineer, I want failed event deliveries to be retried with backoff and ultimately routed to a dead letter queue, so that transient failures are recovered automatically and persistent failures are isolated for investigation.

#### Acceptance Criteria

1. THE EDA_System SHALL apply a per-event-type Retry_Policy with configurable: `initial_delay_ms`, `backoff_multiplier`, `max_delay_ms`, and `max_attempts`.
2. WHEN a Consumer_Worker handler fails and the `dispatch_attempts` count is below `max_attempts`, THE EDA_System SHALL requeue the Domain_Event with a delay calculated by the Retry_Policy.
3. WHEN a Consumer_Worker handler fails and the `dispatch_attempts` count equals `max_attempts`, THE EDA_System SHALL route the Domain_Event to the Dead_Letter_Queue and SHALL emit a structured log event containing the event ID, event type, aggregate ID, attempt count, and final error.
4. THE EDA_System SHALL expose a Prometheus counter `eda_dead_letter_events_total` labelled by `event_type`, incremented each time a Domain_Event is routed to the Dead_Letter_Queue.
5. THE EDA_System SHALL provide an admin API endpoint `POST /api/admin/eda/dead-letter/:event_id/replay` that re-publishes a specific Dead_Letter_Queue event back to its original topic for reprocessing.
6. IF the replay endpoint is called for an event ID that does not exist in the Dead_Letter_Queue, THEN THE EDA_System SHALL return HTTP 404.

---

### Requirement 6: Compliance Check Worker

**User Story:** As a compliance officer, I want AML/KYC compliance checks to be triggered asynchronously by payment events, so that compliance processing does not block the payment initiation response.

#### Acceptance Criteria

1. WHEN a `PaymentInitiated` Domain_Event is published, THE Compliance_Worker SHALL consume the event and execute the configured AML/KYC compliance check for the associated transaction.
2. WHEN the compliance check passes, THE Compliance_Worker SHALL publish a `ComplianceCheckPassed` Domain_Event containing the transaction ID, check type, and timestamp.
3. WHEN the compliance check fails, THE Compliance_Worker SHALL publish a `ComplianceCheckFailed` Domain_Event containing the transaction ID, failure reason, and timestamp, and SHALL update the transaction status in the database.
4. WHEN a `ComplianceCheckFailed` event is published, THE EDA_System SHALL trigger the Notification_Worker to dispatch an alert to the compliance team.
5. THE Compliance_Worker SHALL complete processing of a `PaymentInitiated` event within a configurable SLA duration and SHALL emit a Prometheus histogram observation for each processing duration.

---

### Requirement 7: Partner Notification Worker

**User Story:** As a platform operator, I want partner and user notifications to be dispatched asynchronously via domain events, so that notification failures do not affect core payment processing.

#### Acceptance Criteria

1. WHEN a `NotificationRequested` Domain_Event is published, THE Notification_Worker SHALL consume the event and dispatch the notification to the specified channel (email, SMS, or in-app).
2. WHEN a notification is dispatched successfully, THE Notification_Worker SHALL publish a `NotificationDelivered` Domain_Event containing the notification ID, channel, and timestamp.
3. WHEN a notification dispatch fails, THE Notification_Worker SHALL apply the Retry_Policy for the `NotificationRequested` event type before routing to the Dead_Letter_Queue.
4. THE Notification_Worker SHALL record the delivery status of every notification attempt in the database using the existing notification persistence layer.
5. THE EDA_System SHALL expose a Prometheus counter `eda_notifications_dispatched_total` labelled by `channel` and `status`, incremented on each notification dispatch attempt.

---

### Requirement 8: Webhook Dispatch Worker

**User Story:** As a partner developer, I want webhook events to be delivered reliably to registered endpoints, so that my integration receives real-time updates on payment and compliance state changes.

#### Acceptance Criteria

1. WHEN a `WebhookEventTriggered` Domain_Event is published, THE Webhook_Worker SHALL consume the event and deliver an HTTP POST request to the registered partner endpoint with the event payload.
2. THE Webhook_Worker SHALL sign every outbound webhook payload using HMAC-SHA256 with the partner's registered webhook secret and include the signature in the `X-Aframp-Signature` request header.
3. WHEN the partner endpoint returns HTTP 2xx, THE Webhook_Worker SHALL publish a `WebhookDelivered` Domain_Event and record the delivery as successful.
4. WHEN the partner endpoint returns a non-2xx response or the request times out, THE Webhook_Worker SHALL apply the Retry_Policy for the `WebhookEventTriggered` event type.
5. WHEN a webhook delivery is routed to the Dead_Letter_Queue after exhausting retries, THE EDA_System SHALL notify the platform operations team via the Notification_Worker.
6. THE EDA_System SHALL expose a Prometheus histogram `eda_webhook_delivery_duration_seconds` labelled by `partner_id` and `status`, recording the HTTP round-trip duration for each delivery attempt.

---

### Requirement 9: Ledger Entry Worker

**User Story:** As a finance engineer, I want ledger entries to be queued and submitted to the Stellar network asynchronously, so that ledger settlement does not block payment API responses.

#### Acceptance Criteria

1. WHEN a `LedgerEntryQueued` Domain_Event is published, THE Ledger_Worker SHALL consume the event and submit the corresponding Stellar transaction to the Horizon API.
2. WHEN the Stellar transaction is confirmed, THE Ledger_Worker SHALL publish a `LedgerEntrySettled` Domain_Event containing the transaction hash, ledger sequence, and timestamp.
3. WHEN the Stellar transaction fails with a retriable error, THE Ledger_Worker SHALL apply the Retry_Policy for the `LedgerEntryQueued` event type.
4. WHEN the Stellar transaction fails with a non-retriable error (e.g. insufficient balance, invalid sequence), THE Ledger_Worker SHALL publish a `LedgerEntryFailed` Domain_Event and route the original event to the Dead_Letter_Queue.
5. THE Ledger_Worker SHALL enforce idempotency using the `event_id` as the Stellar transaction memo to prevent duplicate on-chain submissions.
6. THE EDA_System SHALL expose a Prometheus counter `eda_ledger_entries_total` labelled by `status` (`settled` | `failed` | `retried`), incremented on each ledger submission outcome.

---

### Requirement 10: Configuration

**User Story:** As a platform engineer, I want all EDA parameters to be configurable via the existing TOML configuration system, so that broker selection, retry policies, and worker concurrency can be tuned per environment without code changes.

#### Acceptance Criteria

1. THE EDA_System SHALL read its configuration from an `[eda]` section in the environment TOML files, consistent with the existing `[workers]`, `[database]`, and `[cache]` sections.
2. THE `[eda]` section SHALL support the following fields: `broker_type` (`rabbitmq` | `kafka`), `broker_url`, `outbox_poll_interval_secs`, `outbox_batch_size`, `idempotency_ttl_secs`, and a `[eda.retry_policies]` subsection keyed by event type.
3. THE EDA_System SHALL validate the `[eda]` configuration at startup and SHALL return a descriptive error and refuse to start if any required field is missing or invalid.
4. WHERE the `broker_type` is `rabbitmq`, THE `[eda]` section SHALL additionally support `exchange_name`, `prefetch_count`, and `connection_pool_size`.
5. WHERE the `broker_type` is `kafka`, THE `[eda]` section SHALL additionally support `bootstrap_servers`, `consumer_group_id`, and `auto_offset_reset`.

---

### Requirement 11: Observability

**User Story:** As a platform operator, I want Prometheus metrics, structured log events, and distributed trace spans for all EDA activity, so that I can monitor throughput, latency, error rates, and end-to-end event flows in real time.

#### Acceptance Criteria

1. THE EDA_System SHALL expose a Prometheus counter `eda_events_published_total` labelled by `event_type` and `broker_type`, incremented on each successful publish to the Message_Broker.
2. THE EDA_System SHALL expose a Prometheus counter `eda_events_consumed_total` labelled by `event_type`, `worker`, and `status` (`success` | `error` | `duplicate_skipped`), incremented on each Consumer_Worker processing outcome.
3. THE EDA_System SHALL expose a Prometheus histogram `eda_outbox_relay_lag_seconds` recording the duration between `created_at` and `dispatched_at` for each Outbox_Table row.
4. THE EDA_System SHALL expose a Prometheus gauge `eda_outbox_pending_count` reflecting the current count of Outbox_Table rows with `status = 'pending'`, updated on each Outbox_Relay poll cycle.
5. THE EDA_System SHALL propagate the `Correlation_ID` from each Domain_Event into the OpenTelemetry trace context so that the full event processing chain is visible as a single distributed trace.
6. THE EDA_System SHALL emit a structured log event for every Outbox_Table row that transitions to `status = 'failed'`, containing the event ID, event type, aggregate ID, attempt count, and error message.
7. THE EDA_System SHALL emit a structured log event for every Dead_Letter_Queue routing, containing the event ID, event type, worker name, attempt count, and final error.
8. WHEN the `eda_outbox_pending_count` gauge exceeds a configurable threshold for longer than a configurable duration, THE EDA_System SHALL fire a Prometheus alert.

---

### Requirement 12: Security

**User Story:** As a security engineer, I want all broker connections and event payloads to be protected, so that sensitive financial data is not exposed in transit or at rest in the message broker.

#### Acceptance Criteria

1. THE EDA_System SHALL require TLS for all connections to the Message_Broker in production and staging environments.
2. THE EDA_System SHALL authenticate to the Message_Broker using credentials injected via environment variables, never hardcoded in configuration files.
3. THE EDA_System SHALL encrypt sensitive fields within the `payload` of an Event_Envelope (e.g. account numbers, amounts) using the existing AES-GCM payload encryption layer before writing to the Outbox_Table.
4. THE EDA_System SHALL validate that every inbound Event_Envelope received by a Consumer_Worker has a recognised `event_type` and `schema_version` before passing it to the handler.
5. IF an inbound Event_Envelope fails validation, THEN THE Consumer_Worker SHALL reject the message without processing, route it to the Dead_Letter_Queue, and emit a structured log event containing the raw envelope and the validation error.
6. THE EDA_System SHALL apply the principle of least privilege: each Consumer_Worker SHALL only have subscribe permissions on the topics it is registered to consume.

---

### Requirement 13: Unit Tests

**User Story:** As a developer, I want unit tests for core EDA logic, so that regressions in outbox writing, idempotency checks, retry calculations, and schema validation are caught before deployment.

#### Acceptance Criteria

1. THE Test_Suite SHALL include unit tests verifying that the `OutboxWriter` inserts an Event_Envelope row within a transaction and that a rollback leaves no row in the Outbox_Table.
2. THE Test_Suite SHALL include unit tests verifying that the `IdempotencyStore` correctly identifies duplicate `event_id` values and that non-duplicate IDs are processed.
3. THE Test_Suite SHALL include unit tests verifying that the Retry_Policy delay calculation produces the correct sequence of delays for a given `initial_delay_ms`, `backoff_multiplier`, and `max_delay_ms`.
4. THE Test_Suite SHALL include unit tests verifying that the Schema_Registry rejects Event_Envelopes with unknown `event_type`, unknown `schema_version`, missing required fields, and incompatible schema changes.
5. THE Test_Suite SHALL include a property-based test verifying the round-trip property: FOR ALL valid Event_Envelopes, `deserialise(serialise(envelope)) == envelope`.
6. THE Test_Suite SHALL include unit tests verifying that the `Broker_Adapter` trait implementations correctly map publish errors to typed `EDA_System` error variants.

---

### Requirement 14: Integration Tests

**User Story:** As a developer, I want integration tests covering the full event lifecycle, so that end-to-end correctness of the transactional outbox, broker delivery, and worker processing is verified.

#### Acceptance Criteria

1. THE Test_Suite SHALL include an integration test verifying the full outbox-to-broker path: a business transaction writes an Event_Envelope to the Outbox_Table, the Outbox_Relay publishes it to the Message_Broker, and a Consumer_Worker processes it successfully.
2. THE Test_Suite SHALL include an integration test verifying that a broker outage during Outbox_Relay polling does not lose events — rows remain `status = 'pending'` and are successfully published when the broker recovers.
3. THE Test_Suite SHALL include an integration test verifying that duplicate event delivery is handled correctly: a Consumer_Worker receiving the same `event_id` twice processes the handler exactly once.
4. THE Test_Suite SHALL include an integration test verifying that a Consumer_Worker handler failure triggers the Retry_Policy and that exhausting retries routes the event to the Dead_Letter_Queue.
5. THE Test_Suite SHALL include an integration test verifying that the `POST /api/admin/eda/dead-letter/:event_id/replay` endpoint re-publishes the event and the Consumer_Worker processes it successfully.
6. THE Test_Suite SHALL include an integration test verifying that the Compliance_Worker, Notification_Worker, Webhook_Worker, and Ledger_Worker each correctly process their respective Domain_Events end-to-end using a test broker instance.
