//! Optional token-owner metadata. Failures leave relevance unknown, not false,
//! and must never prevent normal repository monitoring.
use super::*;
use crate::domain::{Relevance, RelevanceReason};
use serde_json::{Value, json};
use std::collections::HashMap;

async fn query(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    query: &str,
    variables: Value,
) -> Option<Value> {
    let response = client
        .client
        .post(GitHubClient::graphql_url(configuration).ok()?)
        .bearer_auth(token.expose())
        .header(ACCEPT, ACCEPT_VALUE)
        .header(USER_AGENT_HEADER, USER_AGENT)
        .header("X-GitHub-Api-Version", API_VERSION)
        .json(&json!({"query": query, "variables": variables}))
        .send()
        .await
        .ok()?;
    if response.status() != StatusCode::OK {
        return None;
    }
    let response: GitHubGraphQlResponse<Value> = response.json().await.ok()?;
    if !response.errors.is_empty() {
        report_graphql_errors(&response.errors);
        return None;
    }
    response.data
}

fn flag(value: &Value, path: &str) -> bool {
    value.pointer(path).and_then(Value::as_bool) == Some(true)
}

fn any_viewer(connection: &Value, field: &str) -> bool {
    connection["nodes"]
        .as_array()
        .is_some_and(|nodes| nodes.iter().any(|node| flag(node, field)))
}

fn reason(relevance: &mut Relevance, matched: bool, reason: RelevanceReason) {
    if matched && !relevance.reasons.contains(&reason) {
        relevance.reasons.push(reason);
    }
}

fn item_relevance(node: &Value) -> Relevance {
    let mut result = Relevance {
        complete: node["viewerDidAuthor"].is_boolean(),
        ..Default::default()
    };
    reason(
        &mut result,
        flag(node, "/viewerDidAuthor"),
        RelevanceReason::Authored,
    );
    match node["__typename"].as_str() {
        Some("PullRequest") => {
            reason(
                &mut result,
                node["viewerLatestReview"]["state"]
                    .as_str()
                    .is_some_and(|state| state != "PENDING"),
                RelevanceReason::Reviewed,
            );
            reason(
                &mut result,
                node["viewerLatestReviewRequest"].is_object(),
                RelevanceReason::ReviewRequested,
            );
            result.complete &= node.get("viewerLatestReview").is_some()
                && node.get("viewerLatestReviewRequest").is_some()
                && node["viewerLatestReview"]["state"] != "PENDING";
        }
        Some("Issue") => {
            reason(
                &mut result,
                node["viewerSubscription"] == "SUBSCRIBED",
                RelevanceReason::Subscribed,
            );
            reason(
                &mut result,
                any_viewer(&node["participants"], "/isViewer"),
                RelevanceReason::Discussed,
            );
            result.complete &= node["viewerSubscription"].is_string()
                && node["participants"]["pageInfo"]["hasNextPage"] == false;
        }
        _ => result.complete = false,
    }
    result
}

pub(super) async fn items(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    ids: &[String],
) -> HashMap<String, Relevance> {
    const QUERY: &str = r#"query Relevance($ids: [ID!]!) {
      viewer { databaseId login }
      nodes(ids: $ids) {
        __typename id
        ... on PullRequest {
          viewerDidAuthor
          viewerLatestReview { state }
          viewerLatestReviewRequest { id }
        }
        ... on Issue {
          viewerDidAuthor viewerSubscription
          participants(first: 100) { nodes { isViewer } pageInfo { hasNextPage endCursor } }
        }
      }
    }"#;
    let mut results = HashMap::new();
    for chunk in ids.chunks(20) {
        let Some(data) = query(client, configuration, token, QUERY, json!({"ids":chunk})).await
        else {
            eprintln!("GitHub personal relevance unavailable; items remain unverified");
            break;
        };
        for node in data["nodes"].as_array().into_iter().flatten() {
            let Some(id) = node["id"].as_str() else {
                continue;
            };
            let mut relevance = item_relevance(node);
            relevance.account_id = data["viewer"]["databaseId"]
                .as_u64()
                .map(|id| id.to_string());
            // A new draft review must not hide an earlier submitted review.
            if node["__typename"] == "PullRequest"
                && node["viewerLatestReview"]["state"] == "PENDING"
                && let Some(login) = data["viewer"]["login"].as_str()
                && let Some(review_data) = query(client, configuration, token,
                    "query SubmittedReviews($id: ID!, $login: String!) { node(id: $id) { ... on PullRequest { reviews(first: 1, author: $login, states: [APPROVED, CHANGES_REQUESTED, COMMENTED, DISMISSED]) { totalCount } } } }",
                    json!({"id": id, "login": login})).await
                && let Some(count) = review_data["node"]["reviews"]["totalCount"].as_u64()
            {
                reason(&mut relevance, count > 0, RelevanceReason::Reviewed);
                relevance.complete = true;
            }
            // Fully page discussion participants before declaring a non-match.
            if node["__typename"] == "Issue" && !relevance.matches() && !relevance.complete {
                let mut page = node["participants"].clone();
                for _ in 0..100 {
                    if page["pageInfo"]["hasNextPage"] != true {
                        break;
                    }
                    let Some(after) = page["pageInfo"]["endCursor"].as_str() else {
                        break;
                    };
                    let Some(data) = query(client, configuration, token,
                        "query Participants($id: ID!, $after: String!) { node(id: $id) { ... on Issue { participants(first: 100, after: $after) { nodes { isViewer } pageInfo { hasNextPage endCursor } } } } }",
                        json!({"id":id,"after":after})).await else { break; };
                    page = data["node"]["participants"].clone();
                    reason(
                        &mut relevance,
                        any_viewer(&page, "/isViewer"),
                        RelevanceReason::Discussed,
                    );
                    relevance.complete = node["viewerSubscription"].is_string()
                        && page["pageInfo"]["hasNextPage"] == false;
                    if relevance.matches() || relevance.complete {
                        break;
                    }
                }
            }
            results.insert(id.to_owned(), relevance);
        }
    }
    results
}

fn commit_relevance(commit: &Value, pulls: &[&Value]) -> Relevance {
    let mut relevance = Relevance {
        complete: commit["author"]["user"]["isViewer"].is_boolean()
            && commit["associatedPullRequests"]["pageInfo"]["hasNextPage"] == false
            && pulls
                .iter()
                .all(|pull| pull["viewerDidAuthor"].is_boolean()),
        ..Default::default()
    };
    reason(
        &mut relevance,
        flag(commit, "/author/user/isViewer"),
        RelevanceReason::CommitAuthored,
    );
    reason(
        &mut relevance,
        any_viewer(&commit["associatedPullRequests"], "/viewerDidAuthor")
            || pulls.iter().any(|pull| flag(pull, "/viewerDidAuthor")),
        RelevanceReason::ChangeRequestAuthored,
    );
    relevance
}

pub(super) async fn runs(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    repository: &Repository,
    runs: &mut [GitHubWorkflowRun],
) {
    // Many workflows share the same commit. Resolve each commit/PR combination once.
    let mut keys = runs
        .iter()
        .map(|run| {
            (
                run.head_sha.clone(),
                run.pull_requests
                    .iter()
                    .map(|pull| pull.number)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    let mut results = HashMap::new();
    for chunk in keys.chunks(20) {
        let mut variables = json!({"owner": repository.owner, "name": repository.name});
        let mut declarations = String::from("$owner: String!, $name: String!");
        let mut fields = String::new();
        for (index, (sha, pulls)) in chunk.iter().enumerate() {
            variables[format!("sha{index}")] = json!(sha);
            declarations.push_str(&format!(", $sha{index}: String!"));
            fields.push_str(&format!("c{index}: object(expression: $sha{index}) {{ ... on Commit {{ author {{ user {{ isViewer }} }} associatedPullRequests(first: 100) {{ nodes {{ viewerDidAuthor }} pageInfo {{ hasNextPage }} }} }} }} "));
            for (pull_index, number) in pulls.iter().enumerate() {
                fields.push_str(&format!(
                    "p{index}_{pull_index}: pullRequest(number: {number}) {{ viewerDidAuthor }} "
                ));
            }
        }
        let statement = format!(
            "query RunRelevance({declarations}) {{ viewer {{ databaseId }} repository(owner: $owner, name: $name) {{ {fields} }} }}"
        );
        let Some(data) = query(client, configuration, token, &statement, variables).await else {
            eprintln!("GitHub workflow ownership unavailable; runs remain unverified");
            break;
        };
        for (index, key) in chunk.iter().enumerate() {
            let repository = &data["repository"];
            let pulls = key
                .1
                .iter()
                .enumerate()
                .map(|(pull_index, _)| &repository[format!("p{index}_{pull_index}")])
                .collect::<Vec<_>>();
            let mut relevance = commit_relevance(&repository[format!("c{index}")], &pulls);
            relevance.account_id = data["viewer"]["databaseId"]
                .as_u64()
                .map(|id| id.to_string());
            results.insert(key.clone(), relevance);
        }
    }
    for run in runs {
        let key = (
            run.head_sha.clone(),
            run.pull_requests
                .iter()
                .map(|pull| pull.number)
                .collect::<Vec<_>>(),
        );
        run.relevance = results.get(&key).cloned().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests;
