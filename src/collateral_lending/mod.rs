/// Collateralized Lending Integration (Issue #379)
///
/// Allows users to borrow fiat or other assets using their cNGN balance as collateral
/// without selling their position. Enforces strict collateral ratio requirements,
/// monitors positions for liquidation risk, and provides transparent borrowing UX.
pub mod models;
pub mod service;

pub use models::*;
pub use service::CollateralLendingService;
