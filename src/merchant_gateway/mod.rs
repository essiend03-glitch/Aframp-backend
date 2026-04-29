// Merchant Gateway Module
// Entry point for commercial adoption - enables businesses to accept cNGN payments

pub mod models;
pub mod repository;
pub mod service;
pub mod handlers;
pub mod routes;
pub mod webhook_engine;
pub mod api_key_service;
pub mod loyalty;
pub mod metrics;
pub mod webhook_queue;

#[cfg(test)]
mod tests;
