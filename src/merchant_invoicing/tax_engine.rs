//! Dynamic tax engine — applies merchant-configured tax rules to line items.
//!
//! Supports both tax-inclusive and tax-exclusive pricing models.

use crate::merchant_invoicing::models::{LineItem, TaxBreakdownEntry, TaxCalculationResult, TaxRule};

/// Calculate tax for a set of line items given the applicable rules.
///
/// Rules are pre-filtered by region and active status before calling this.
pub fn calculate_tax(
    line_items: &[LineItem],
    rules: &[TaxRule],
) -> TaxCalculationResult {
    let gross_subtotal: f64 = line_items
        .iter()
        .map(|item| item.quantity * item.unit_price)
        .sum();

    let mut tax_breakdown: Vec<TaxBreakdownEntry> = Vec::new();
    let mut total_tax = 0.0;

    for rule in rules {
        let rate = rule.rate_bps as f64 / 10_000.0; // basis points → fraction

        // Filter line items that this rule applies to (empty applies_to = all)
        let taxable_amount: f64 = line_items
            .iter()
            .filter(|item| {
                rule.applies_to.is_empty()
                    || item
                        .category
                        .as_deref()
                        .map(|c| rule.applies_to.iter().any(|a| a == c))
                        .unwrap_or(true)
            })
            .map(|item| item.quantity * item.unit_price)
            .sum();

        if taxable_amount == 0.0 {
            continue;
        }

        let tax_amount = if rule.is_inclusive {
            // Tax is already included in the price: tax = amount - amount / (1 + rate)
            taxable_amount - taxable_amount / (1.0 + rate)
        } else {
            // Tax is added on top
            taxable_amount * rate
        };

        let tax_amount = round2(tax_amount);
        total_tax += tax_amount;

        tax_breakdown.push(TaxBreakdownEntry {
            tax_type: rule.tax_type.clone(),
            rate_bps: rule.rate_bps,
            taxable_amount: round2(taxable_amount),
            tax_amount,
        });
    }

    // For inclusive pricing the subtotal is net-of-tax; for exclusive it equals gross
    let has_inclusive = rules.iter().any(|r| r.is_inclusive);
    let subtotal = if has_inclusive {
        round2(gross_subtotal - total_tax)
    } else {
        round2(gross_subtotal)
    };

    TaxCalculationResult {
        subtotal,
        tax_amount: round2(total_tax),
        total_amount: round2(subtotal + total_tax),
        tax_breakdown,
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_rule(rate_bps: i32, is_inclusive: bool) -> TaxRule {
        TaxRule {
            id: Uuid::new_v4(),
            merchant_id: Uuid::new_v4(),
            name: "VAT".into(),
            region: "NG".into(),
            tax_type: "VAT".into(),
            rate_bps,
            is_inclusive,
            applies_to: vec![],
            is_active: true,
            effective_from: Utc::now(),
            effective_until: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_exclusive_vat_7_5_pct() {
        let items = vec![LineItem {
            description: "Product".into(),
            quantity: 1.0,
            unit_price: 10_000.0,
            category: None,
        }];
        let rules = vec![make_rule(750, false)]; // 7.5% exclusive
        let result = calculate_tax(&items, &rules);
        assert_eq!(result.subtotal, 10_000.0);
        assert_eq!(result.tax_amount, 750.0);
        assert_eq!(result.total_amount, 10_750.0);
    }

    #[test]
    fn test_inclusive_vat() {
        let items = vec![LineItem {
            description: "Product".into(),
            quantity: 1.0,
            unit_price: 10_750.0,
            category: None,
        }];
        let rules = vec![make_rule(750, true)]; // 7.5% inclusive
        let result = calculate_tax(&items, &rules);
        // tax = 10750 - 10750/1.075 ≈ 750
        assert!((result.tax_amount - 697.67).abs() < 0.1);
    }

    #[test]
    fn test_no_rules_zero_tax() {
        let items = vec![LineItem {
            description: "Product".into(),
            quantity: 2.0,
            unit_price: 5_000.0,
            category: None,
        }];
        let result = calculate_tax(&items, &[]);
        assert_eq!(result.tax_amount, 0.0);
        assert_eq!(result.subtotal, 10_000.0);
    }
}
