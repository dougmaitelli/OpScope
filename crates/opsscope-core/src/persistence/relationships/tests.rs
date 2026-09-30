use super::*;
use crate::persistence::SqliteDatabase;

#[test]
fn migration_discards_old_decisions_without_losing_resource_data_or_connections() {
    let database = SqliteDatabase::in_memory().unwrap();
    let connection = database.lock().unwrap();
    connection.execute_batch(
        r#"ALTER TABLE workflow_runs ADD COLUMN relevance TEXT NOT NULL DEFAULT '{}';
         ALTER TABLE change_requests ADD COLUMN relevance TEXT NOT NULL DEFAULT '{}';
         INSERT INTO connections (id, provider, account_id, login, profile_url, secret_reference)
            VALUES ('source', 'github', '42', 'alice', '', 'credential');
         INSERT INTO repository_selections (source_id, repository_id) VALUES ('source', 'repo');
         INSERT INTO issue_cache (source_id, account_id, repository_id, refreshed_at, payload)
            VALUES ('source', '42', 'repo', 100, '[{"id":"issue","title":"Keep me","relevance":{"account_id":"42","reasons":["Authored"],"complete":true}}]');
         INSERT INTO work_item_notification_states (source_id, account_id, repository_id, snapshot)
            VALUES ('source', '42', 'repo', '{"pull_requests":[{"id":"pr","relevance":{"reasons":["ReviewRequested"]}}]}');"#
    ).unwrap();
    migrate(&connection).unwrap();
    migrate(&connection).unwrap();
    let (age, payload): (i64, String) = connection
        .query_row("SELECT refreshed_at, payload FROM issue_cache", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(age, 0);
    let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(value[0]["title"], "Keep me");
    assert!(value[0].get("relevance").is_none());
    let snapshot: String = connection
        .query_row(
            "SELECT snapshot FROM work_item_notification_states",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!snapshot.contains("relevance"));
    assert!(snapshot.contains("pr"));
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM repository_selections", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT secret_reference FROM connections", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "credential"
    );
    assert!(
        connection
            .prepare("SELECT relevance FROM workflow_runs")
            .is_err()
    );
}
