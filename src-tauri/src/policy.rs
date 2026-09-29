//! Action authorization is independent of providers and model instructions.
//! Callers construct action categories and bindings in Rust from current data.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    Discuss,
    Balanced,
    JustDoIt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Approval {
    Always,
    Important,
    Autonomous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Keep future action boundaries explicit before integrations can use them.
#[allow(dead_code)]
pub enum ActionCategory {
    ReadOnly,
    ReversibleLocalWrite,
    ExternalWrite,
    Destructive,
    Financial,
    CredentialSecuritySensitive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionOrigin {
    UserRequested,
    ModelInitiated,
}

/// Include the base/target revision or other operation arguments in `operation`.
/// Reconstruct this binding from the latest persisted request before execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionBinding {
    pub conversation_id: String,
    pub message_id: String,
    pub operation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub binding: ActionBinding,
    pub category: ActionCategory,
    pub origin: ActionOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    ReviewRequired,
}

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub execution: Execution,
    pub approval: Approval,
}

impl Policy {
    pub fn from_settings(execution: &str, approval: &str) -> Result<Self, String> {
        let execution = match execution {
            "discuss" => Execution::Discuss,
            "balanced" => Execution::Balanced,
            "just_do_it" => Execution::JustDoIt,
            _ => return Err("Execution preference is unavailable.".into()),
        };
        let approval = match approval {
            "always" => Approval::Always,
            "important" => Approval::Important,
            "autonomous" => Approval::Autonomous,
            _ => return Err("Approval preference is unavailable.".into()),
        };
        Ok(Self {
            execution,
            approval,
        })
    }

    pub fn evaluate(&self, action: &Action) -> Decision {
        use ActionCategory::*;
        if action.category == ReadOnly {
            return Decision::Allow;
        }
        if matches!(
            action.category,
            Destructive | Financial | CredentialSecuritySensitive
        ) || self.approval == Approval::Always
            || self.execution == Execution::Discuss
            || (self.approval == Approval::Important && action.category == ExternalWrite)
            || (self.execution == Execution::Balanced
                && action.origin == ActionOrigin::ModelInitiated)
        {
            Decision::ReviewRequired
        } else {
            Decision::Allow
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub id: String,
    pub binding: ActionBinding,
}

struct StoredAction {
    action: Action,
    created: Instant,
}

/// In-memory approvals deliberately do not survive restart. A grant authorizes
/// one exact action, never a conversation, provider turn, or future operation.
#[derive(Default)]
pub struct ApprovalStore {
    pending: HashMap<String, StoredAction>,
    grants: HashMap<String, StoredAction>,
}

impl ApprovalStore {
    const LIFETIME: Duration = Duration::from_secs(10 * 60);
    const MAX_ENTRIES: usize = 128;

    fn prune(&mut self) {
        self.pending
            .retain(|_, entry| entry.created.elapsed() < Self::LIFETIME);
        self.grants
            .retain(|_, entry| entry.created.elapsed() < Self::LIFETIME);
    }

    pub fn register(&mut self, action: Action) -> Result<ReviewRequest, String> {
        self.prune();
        if let Some((id, _)) = self
            .pending
            .iter()
            .find(|(_, entry)| entry.action == action)
        {
            return Ok(ReviewRequest {
                id: id.clone(),
                binding: action.binding,
            });
        }
        self.pending.retain(|_, entry| {
            entry.action.binding.conversation_id != action.binding.conversation_id
        });
        self.grants.retain(|_, entry| {
            entry.action.binding.conversation_id != action.binding.conversation_id
        });
        if self.pending.len() + self.grants.len() >= Self::MAX_ENTRIES {
            return Err("Too many actions await approval. Try again shortly.".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let binding = action.binding.clone();
        self.pending.insert(
            id.clone(),
            StoredAction {
                action,
                created: Instant::now(),
            },
        );
        Ok(ReviewRequest { id, binding })
    }

    pub fn get_binding(&mut self, review_id: &str) -> Result<ActionBinding, String> {
        self.prune();
        self.pending
            .get(review_id)
            .map(|entry| entry.action.binding.clone())
            .ok_or_else(|| "This approval expired. Review the action again.".into())
    }

    /// Invoke only from the explicit UI approval command. The caller supplies a
    /// freshly reconstructed binding, so a newer request invalidates the review.
    pub fn approve(
        &mut self,
        review_id: &str,
        current_binding: &ActionBinding,
    ) -> Result<String, String> {
        self.prune();
        let entry = self
            .pending
            .remove(review_id)
            .ok_or("This approval expired. Review the action again.")?;
        if entry.action.binding != *current_binding {
            return Err("The request changed. Review the action again.".into());
        }
        let token = uuid::Uuid::new_v4().to_string();
        self.grants.insert(
            token.clone(),
            StoredAction {
                action: entry.action,
                created: Instant::now(),
            },
        );
        Ok(token)
    }

    /// Consume before performing any side effect. A mismatched grant is also
    /// consumed, so it cannot be probed or reused with a different operation.
    pub fn authorize(
        &mut self,
        policy: &Policy,
        action: &Action,
        grant: Option<&str>,
    ) -> Result<Decision, String> {
        self.prune();
        if let Some(token) = grant {
            let entry = self
                .grants
                .remove(token)
                .ok_or("This approval expired or was already used. Review the action again.")?;
            if entry.action != *action {
                return Err("The request changed. Review the action again.".into());
            }
            return Ok(Decision::Allow);
        }
        Ok(policy.evaluate(action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(category: ActionCategory, origin: ActionOrigin) -> Action {
        Action {
            binding: ActionBinding {
                conversation_id: "conversation-1".into(),
                message_id: "request-1".into(),
                operation: "website:revise:base=2".into(),
            },
            category,
            origin,
        }
    }

    #[test]
    fn policy_matrix_preserves_sensitive_action_boundary() {
        use ActionCategory::*;
        for execution in [Execution::Discuss, Execution::Balanced, Execution::JustDoIt] {
            for approval in [Approval::Always, Approval::Important, Approval::Autonomous] {
                let policy = Policy {
                    execution,
                    approval,
                };
                for origin in [ActionOrigin::UserRequested, ActionOrigin::ModelInitiated] {
                    assert_eq!(policy.evaluate(&action(ReadOnly, origin)), Decision::Allow);
                    for category in [Destructive, Financial, CredentialSecuritySensitive] {
                        assert_eq!(
                            policy.evaluate(&action(category, origin)),
                            Decision::ReviewRequired
                        );
                    }
                    for category in [ReversibleLocalWrite, ExternalWrite] {
                        let review = approval == Approval::Always
                            || execution == Execution::Discuss
                            || (approval == Approval::Important && category == ExternalWrite)
                            || (execution == Execution::Balanced
                                && origin == ActionOrigin::ModelInitiated);
                        assert_eq!(
                            policy.evaluate(&action(category, origin)),
                            if review {
                                Decision::ReviewRequired
                            } else {
                                Decision::Allow
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_preferences_fail_closed() {
        assert!(Policy::from_settings("unknown", "autonomous").is_err());
        assert!(Policy::from_settings("just_do_it", "unknown").is_err());
    }

    #[test]
    fn exact_grant_is_one_use_and_pending_review_is_not_a_grant() {
        let action = action(
            ActionCategory::ReversibleLocalWrite,
            ActionOrigin::UserRequested,
        );
        let policy = Policy::from_settings("discuss", "always").unwrap();
        let mut store = ApprovalStore::default();
        let review = store.register(action.clone()).unwrap();
        assert!(store.authorize(&policy, &action, Some(&review.id)).is_err());
        assert_eq!(
            store.authorize(&policy, &action, None).unwrap(),
            Decision::ReviewRequired
        );
        let grant = store.approve(&review.id, &action.binding).unwrap();
        assert_eq!(
            store.authorize(&policy, &action, Some(&grant)).unwrap(),
            Decision::Allow
        );
        assert!(store.authorize(&policy, &action, Some(&grant)).is_err());
    }

    #[test]
    fn grants_do_not_authorize_different_requests_or_operations() {
        let original = action(
            ActionCategory::ReversibleLocalWrite,
            ActionOrigin::UserRequested,
        );
        let policy = Policy::from_settings("discuss", "important").unwrap();
        let mut mismatches = Vec::new();
        let mut other = original.clone();
        other.binding.conversation_id = "other".into();
        mismatches.push(other);
        let mut other = original.clone();
        other.binding.message_id = "request-2".into();
        mismatches.push(other);
        let mut other = original.clone();
        other.binding.operation = "website:revise:base=3".into();
        mismatches.push(other);
        let mut other = original.clone();
        other.category = ActionCategory::ExternalWrite;
        mismatches.push(other);
        let mut other = original.clone();
        other.origin = ActionOrigin::ModelInitiated;
        mismatches.push(other);
        for different in mismatches {
            let mut store = ApprovalStore::default();
            let review = store.register(original.clone()).unwrap();
            let grant = store.approve(&review.id, &original.binding).unwrap();
            assert!(store.authorize(&policy, &different, Some(&grant)).is_err());
            assert!(store.authorize(&policy, &original, Some(&grant)).is_err());
        }
    }

    #[test]
    fn changed_or_expired_review_cannot_issue_grant() {
        let action = action(
            ActionCategory::ReversibleLocalWrite,
            ActionOrigin::UserRequested,
        );
        let mut store = ApprovalStore::default();
        let review = store.register(action.clone()).unwrap();
        let mut latest = action.binding.clone();
        latest.message_id = "new-request".into();
        assert!(store.approve(&review.id, &latest).is_err());
        let review = store.register(action.clone()).unwrap();
        store.pending.get_mut(&review.id).unwrap().created =
            Instant::now() - ApprovalStore::LIFETIME;
        assert!(store.approve(&review.id, &action.binding).is_err());
        let review = store.register(action.clone()).unwrap();
        let grant = store.approve(&review.id, &action.binding).unwrap();
        store.grants.get_mut(&grant).unwrap().created = Instant::now() - ApprovalStore::LIFETIME;
        let policy = Policy::from_settings("discuss", "important").unwrap();
        assert!(store.authorize(&policy, &action, Some(&grant)).is_err());
    }

    #[test]
    fn new_review_invalidates_earlier_approval_for_conversation() {
        let original = action(
            ActionCategory::ReversibleLocalWrite,
            ActionOrigin::UserRequested,
        );
        let mut store = ApprovalStore::default();
        let first = store.register(original.clone()).unwrap();
        assert_eq!(store.get_binding(&first.id).unwrap(), original.binding);
        let grant = store.approve(&first.id, &original.binding).unwrap();
        let mut next = original.clone();
        next.binding.message_id = "request-2".into();
        let second = store.register(next).unwrap();
        assert!(store.get_binding(&first.id).is_err());
        let policy = Policy::from_settings("discuss", "important").unwrap();
        assert!(store.authorize(&policy, &original, Some(&grant)).is_err());
        assert!(store.get_binding(&second.id).is_ok());
    }

    #[test]
    fn outstanding_reviews_are_bounded_and_other_conversations_are_preserved() {
        let original = action(
            ActionCategory::ReversibleLocalWrite,
            ActionOrigin::UserRequested,
        );
        let mut store = ApprovalStore::default();
        let first = store.register(original.clone()).unwrap();
        let grant = store.approve(&first.id, &original.binding).unwrap();
        for index in 1..ApprovalStore::MAX_ENTRIES {
            let mut other = original.clone();
            other.binding.conversation_id = format!("conversation-{index}-other");
            store.register(other).unwrap();
        }
        let mut excess = original.clone();
        excess.binding.conversation_id = "excess".into();
        assert!(store.register(excess).is_err());
        let policy = Policy::from_settings("discuss", "important").unwrap();
        assert_eq!(
            store.authorize(&policy, &original, Some(&grant)).unwrap(),
            Decision::Allow
        );
    }
}
