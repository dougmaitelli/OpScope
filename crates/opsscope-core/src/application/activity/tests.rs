use super::*;
use crate::application::{CredentialField, SourceCapability, SourceDescriptor};
use crate::domain::{
    ChangeRequestCheckStatus, ChangeRequestMergeStatus, ChangeRequestReviewStatus,
    ChangeRequestState, RepositoryVisibility,
};
use std::sync::Mutex;

#[derive(Default)]
struct MemoryEvents {
    state: Mutex<Option<Vec<ChangeRequest>>>,
    events: Mutex<Vec<ChangeRequestActivityEvent>>,
}

impl ActivityEventRepository for MemoryEvents {
    fn load_change_request_state(
        &self,
        _source_id: &str,
        _repository_id: &str,
    ) -> Result<Option<Vec<ChangeRequest>>, PersistenceFailure> {
        Ok(self.state.lock().expect("state lock").clone())
    }

    fn save_change_request_observation(
        &self,
        _source_id: &str,
        _repository_id: &str,
        observed: &[ChangeRequest],
        events: &[ChangeRequestActivityEvent],
    ) -> Result<(), PersistenceFailure> {
        *self.state.lock().expect("state lock") = Some(observed.to_vec());
        self.events
            .lock()
            .expect("event lock")
            .extend_from_slice(events);
        Ok(())
    }

    fn list_change_request_events(
        &self,
    ) -> Result<Vec<ChangeRequestActivityEvent>, PersistenceFailure> {
        Ok(self.events.lock().expect("event lock").clone())
    }
}

fn source() -> ConnectedSource {
    ConnectedSource {
        id: "source".to_owned(),
        label: "Source".to_owned(),
        descriptor: SourceDescriptor {
            id: "module".to_owned(),
            name: "Module".to_owned(),
            description: String::new(),
            abbreviation: "MO".to_owned(),
            capabilities: vec![SourceCapability::ChangeRequests],
            credential: CredentialField {
                label: String::new(),
                placeholder: String::new(),
                help: String::new(),
            },
            connection_fields: Vec::new(),
        },
    }
}

fn repository() -> Repository {
    Repository {
        id: "repository".to_owned(),
        owner: "owner".to_owned(),
        name: "project".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://example.com/owner/project".to_owned(),
    }
}

fn change_request() -> ChangeRequest {
    ChangeRequest {
        relevance: Default::default(),
        id: "pr-1".to_owned(),
        number: 1,
        title: "Improve activity".to_owned(),
        author: Some("octocat".to_owned()),
        source_branch: "activity".to_owned(),
        target_branch: "main".to_owned(),
        state: ChangeRequestState::Open,
        draft: true,
        review_status: ChangeRequestReviewStatus::ReviewRequired,
        check_status: ChangeRequestCheckStatus::Running,
        merge_status: ChangeRequestMergeStatus::Ready,
        created_at: "2026-09-26T10:00:00Z".to_owned(),
        updated_at: "2026-09-26T10:00:00Z".to_owned(),
        web_url: "https://example.com/pull/1".to_owned(),
    }
}

#[test]
fn establishes_a_baseline_then_records_only_meaningful_transitions() {
    let events = Arc::new(MemoryEvents::default());
    let tracker = TrackChangeRequestActivity::new(events.clone());
    let original = change_request();
    tracker
        .observe(
            &source(),
            &repository(),
            std::slice::from_ref(&original),
            &[],
        )
        .expect("baseline");

    let mut updated = original.clone();
    updated.draft = false;
    updated.review_status = ChangeRequestReviewStatus::Approved;
    updated.check_status = ChangeRequestCheckStatus::Failing;
    updated.merge_status = ChangeRequestMergeStatus::Conflicting;
    updated.updated_at = "2026-09-27T10:00:00Z".to_owned();
    tracker
        .observe(
            &source(),
            &repository(),
            std::slice::from_ref(&updated),
            &[],
        )
        .expect("transition");
    tracker
        .observe(
            &source(),
            &repository(),
            std::slice::from_ref(&updated),
            &[],
        )
        .expect("unchanged");

    assert_eq!(
        events
            .events
            .lock()
            .expect("event lock")
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        vec![
            ChangeRequestActivityKind::Opened,
            ChangeRequestActivityKind::ReadyForReview,
            ChangeRequestActivityKind::ReviewApproved,
            ChangeRequestActivityKind::ChecksFailed,
            ChangeRequestActivityKind::ConflictDetected,
        ]
    );
}

#[test]
fn records_resolved_departures_as_merged_or_closed() {
    let events = Arc::new(MemoryEvents {
        state: Mutex::new(Some(vec![change_request()])),
        events: Mutex::new(Vec::new()),
    });
    let tracker = TrackChangeRequestActivity::new(events.clone());
    let original = change_request();
    let mut merged = original.clone();
    merged.state = ChangeRequestState::Merged;
    merged.updated_at = "2026-09-27T12:00:00Z".to_owned();

    tracker
        .observe(&source(), &repository(), &[], &[merged])
        .expect("departure");

    assert_eq!(
        events.events.lock().expect("event lock")[0].kind,
        ChangeRequestActivityKind::Merged
    );
}

#[test]
fn retains_an_unresolved_departure_for_the_next_observation() {
    let original = change_request();
    let events = Arc::new(MemoryEvents {
        state: Mutex::new(Some(vec![original.clone()])),
        events: Mutex::new(Vec::new()),
    });
    let tracker = TrackChangeRequestActivity::new(events.clone());

    tracker
        .observe(&source(), &repository(), &[], &[])
        .expect("unresolved departure");

    assert_eq!(
        tracker.previous("source", "repository").expect("state"),
        Some(vec![original])
    );
    assert!(events.events.lock().expect("event lock").is_empty());
}
