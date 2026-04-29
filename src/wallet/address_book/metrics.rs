use super::models::AddressEntryType;
use prometheus::{Counter, CounterVec, Gauge, GaugeVec, Opts, Registry};
use std::sync::Arc;

pub struct AddressBookMetrics {
    entries_created: CounterVec,
    verification_events: CounterVec,
    verification_failures: CounterVec,
    import_events: Counter,
    export_events: Counter,
    total_entries: GaugeVec,
    stale_verifications: Gauge,
}

impl AddressBookMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let entries_created = CounterVec::new(
            Opts::new(
                "address_book_entries_created_total",
                "Total number of address book entries created by type",
            ),
            &["entry_type"],
        )?;
        registry.register(Box::new(entries_created.clone()))?;

        let verification_events = CounterVec::new(
            Opts::new(
                "address_book_verification_events_total",
                "Total number of verification events by type",
            ),
            &["entry_type", "success"],
        )?;
        registry.register(Box::new(verification_events.clone()))?;

        let verification_failures = CounterVec::new(
            Opts::new(
                "address_book_verification_failures_total",
                "Total number of verification failures by type",
            ),
            &["entry_type"],
        )?;
        registry.register(Box::new(verification_failures.clone()))?;

        let import_events = Counter::with_opts(Opts::new(
            "address_book_import_events_total",
            "Total number of CSV import events",
        ))?;
        registry.register(Box::new(import_events.clone()))?;

        let export_events = Counter::with_opts(Opts::new(
            "address_book_export_events_total",
            "Total number of CSV export events",
        ))?;
        registry.register(Box::new(export_events.clone()))?;

        let total_entries = GaugeVec::new(
            Opts::new(
                "address_book_total_entries",
                "Total number of address book entries by type across all wallets",
            ),
            &["entry_type"],
        )?;
        registry.register(Box::new(total_entries.clone()))?;

        let stale_verifications = Gauge::with_opts(Opts::new(
            "address_book_stale_verifications",
            "Number of entries with stale verification status",
        ))?;
        registry.register(Box::new(stale_verifications.clone()))?;

        Ok(Self {
            entries_created,
            verification_events,
            verification_failures,
            import_events,
            export_events,
            total_entries,
            stale_verifications,
        })
    }

    pub fn record_entry_created(&self, entry_type: AddressEntryType) {
        self.entries_created
            .with_label_values(&[&entry_type.to_string()])
            .inc();
    }

    pub fn record_verification_event(&self, entry_type: AddressEntryType, success: bool) {
        self.verification_events
            .with_label_values(&[&entry_type.to_string(), if success { "true" } else { "false" }])
            .inc();

        if !success {
            self.verification_failures
                .with_label_values(&[&entry_type.to_string()])
                .inc();
        }
    }

    pub fn record_import_event(&self) {
        self.import_events.inc();
    }

    pub fn record_export_event(&self) {
        self.export_events.inc();
    }

    pub fn set_total_entries(&self, entry_type: AddressEntryType, count: f64) {
        self.total_entries
            .with_label_values(&[&entry_type.to_string()])
            .set(count);
    }

    pub fn set_stale_verifications(&self, count: f64) {
        self.stale_verifications.set(count);
    }
}
