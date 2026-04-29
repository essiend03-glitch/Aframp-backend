use crate::negotiation::models::{NegotiationSession, NegotiationState};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct NegotiationRepository {
    store: Arc<RwLock<HashMap<Uuid, NegotiationSession>>>,
}

impl NegotiationRepository {
    pub async fn save(&self, session: NegotiationSession) {
        self.store.write().await.insert(session.id, session);
    }

    pub async fn get(&self, id: Uuid) -> Option<NegotiationSession> {
        self.store.read().await.get(&id).cloned()
    }

    pub async fn update(&self, session: NegotiationSession) {
        self.store.write().await.insert(session.id, session);
    }

    /// Remove all sessions in the Failed state (garbage collection).
    pub async fn gc_failed(&self) -> usize {
        let mut store = self.store.write().await;
        let before = store.len();
        store.retain(|_, s| s.state != NegotiationState::Failed);
        before - store.len()
    }
}
