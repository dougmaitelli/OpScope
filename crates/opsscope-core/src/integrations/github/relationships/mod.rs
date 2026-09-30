//! Optional token-owner metadata. Failures leave relevance unknown, not false,
//! and must never prevent normal repository monitoring.
use super::*;
use crate::domain::{AccountSet, Relationships, ViewerRelationships};
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
        .graphql_request(
            configuration,
            token,
            json!({"query": query, "variables": variables}),
        )
        .ok()?
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

fn ids(connection: &Value, path: &str) -> AccountSet {
    let nodes = connection["nodes"].as_array();
    let values: Vec<_> = nodes
        .into_iter()
        .flatten()
        .filter_map(|node| node.pointer(path).and_then(Value::as_u64))
        .map(|id| id.to_string())
        .collect();
    AccountSet::new(
        values.clone(),
        nodes.is_some_and(|nodes| nodes.len() == values.len())
            && connection["pageInfo"]["hasNextPage"] == false,
    )
}

fn item_relationships(node: &Value, account: &str) -> Relationships {
    let author = node["author"]["databaseId"]
        .as_u64()
        .map(|id| id.to_string());
    let mut result = Relationships {
        authors: AccountSet::new(author.clone(), author.is_some()),
        ..Default::default()
    };
    let mut viewer = ViewerRelationships {
        account_id: account.into(),
        authored: node["viewerDidAuthor"].as_bool(),
        ..Default::default()
    };
    match node["__typename"].as_str() {
        Some("PullRequest") => {
            viewer.reviewed = node.get("viewerLatestReview").and_then(|v| {
                if v.is_null() {
                    Some(false)
                } else {
                    v["state"]
                        .as_str()
                        .filter(|state| *state != "PENDING")
                        .map(|_| true)
                }
            });
            viewer.review_requested = node.get("viewerLatestReviewRequest").map(Value::is_object);
            let mut reviews = node["reviews"].clone();
            if let Some(nodes) = reviews["nodes"].as_array_mut() {
                nodes.retain(|node| {
                    node["state"]
                        .as_str()
                        .is_some_and(|state| state != "PENDING")
                });
            }
            result.reviewers = ids(&reviews, "/author/databaseId");
            result.requested_reviewers =
                ids(&node["reviewRequests"], "/requestedReviewer/databaseId");
        }
        Some("Issue") => {
            viewer.subscribed = node["viewerSubscription"]
                .as_str()
                .map(|v| v == "SUBSCRIBED");
            viewer.discussed = if any_viewer(&node["participants"], "/isViewer") {
                Some(true)
            } else if node["participants"]["pageInfo"]["hasNextPage"] == false {
                Some(false)
            } else {
                None
            };
            result.participants = ids(&node["participants"], "/databaseId");
        }
        _ => {}
    }
    if !account.is_empty() {
        result.viewers.push(viewer);
    }
    result
}

pub(super) async fn items(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    ids: &[String],
) -> HashMap<String, Relationships> {
    const QUERY: &str = r#"query Relationships($ids: [ID!]!) {
      viewer { databaseId login }
      nodes(ids: $ids) {
        __typename id
        ... on PullRequest {
          author { ... on User { databaseId } }
          reviews(first: 100) { nodes { state author { ... on User { databaseId } } } pageInfo { hasNextPage endCursor } }
          reviewRequests(first: 100) { nodes { requestedReviewer { ... on User { databaseId } } } pageInfo { hasNextPage endCursor } }
          viewerDidAuthor
          viewerLatestReview { state }
          viewerLatestReviewRequest { id }
        }
        ... on Issue {
          author { ... on User { databaseId } }
          viewerDidAuthor viewerSubscription
          participants(first: 100) { nodes { databaseId isViewer } pageInfo { hasNextPage endCursor } }
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
            let account = data["viewer"]["databaseId"]
                .as_u64()
                .map(|id| id.to_string())
                .unwrap_or_default();
            let mut node = node.clone();
            // Gather actor evidence irrespective of whether the current viewer already matches.
            for (field, selection) in [
                ("participants", "databaseId isViewer"),
                ("reviews", "state author { ... on User { databaseId } }"),
                (
                    "reviewRequests",
                    "requestedReviewer { ... on User { databaseId } }",
                ),
            ] {
                let mut page = node[field].clone();
                for _ in 0..100 {
                    if page["pageInfo"]["hasNextPage"] != true {
                        break;
                    }
                    let Some(after) = page["pageInfo"]["endCursor"].as_str() else {
                        break;
                    };
                    let typename = node["__typename"].as_str().unwrap_or("");
                    if !matches!(typename, "Issue" | "PullRequest") {
                        break;
                    }
                    let statement = format!(
                        "query RelationshipPage($id: ID!, $after: String!) {{ node(id: $id) {{ ... on {typename} {{ {field}(first: 100, after: $after) {{ nodes {{ {selection} }} pageInfo {{ hasNextPage endCursor }} }} }} }} }}"
                    );
                    let Some(next) = query(
                        client,
                        configuration,
                        token,
                        &statement,
                        json!({"id": id, "after": after}),
                    )
                    .await
                    else {
                        break;
                    };
                    page = next["node"][field].clone();
                    if let Some(values) = page["nodes"].as_array() {
                        if let Some(all) = node[field]["nodes"].as_array_mut() {
                            all.extend(values.clone());
                        }
                    } else {
                        break;
                    }
                    node[field]["pageInfo"] = page["pageInfo"].clone();
                }
            }
            let mut relevance = item_relationships(&node, &account);
            if node["__typename"] == "PullRequest"
                && node["viewerLatestReview"]["state"] == "PENDING"
                && let Some(login) = data["viewer"]["login"].as_str()
                && let Some(review_data) = query(client, configuration, token,
                    "query SubmittedReviews($id: ID!, $login: String!) { node(id: $id) { ... on PullRequest { reviews(first: 1, author: $login, states: [APPROVED, CHANGES_REQUESTED, COMMENTED, DISMISSED]) { totalCount } } } }",
                    json!({"id": id, "login": login})).await
                && let Some(count) = review_data["node"]["reviews"]["totalCount"].as_u64()
                && let Some(viewer) = relevance.viewers.first_mut()
            {
                viewer.reviewed = Some(count > 0);
            }
            results.insert(id.to_owned(), relevance);
        }
    }
    results
}

fn commit_relationships(commit: &Value, pulls: &[&Value], account: &str) -> Relationships {
    let author = commit["author"]["user"]["databaseId"]
        .as_u64()
        .map(|id| id.to_string());
    let mut associated = ids(&commit["associatedPullRequests"], "/author/databaseId");
    associated.ids.extend(
        pulls
            .iter()
            .filter_map(|pull| pull["author"]["databaseId"].as_u64())
            .map(|id| id.to_string()),
    );
    associated.complete &= pulls
        .iter()
        .all(|pull| pull["author"]["databaseId"].is_u64());
    let pr_match = any_viewer(&commit["associatedPullRequests"], "/viewerDidAuthor")
        || pulls.iter().any(|pull| flag(pull, "/viewerDidAuthor"));
    Relationships {
        linked_change_requests: commit["associatedPullRequests"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(pulls.iter().copied())
            .map(|pull| crate::domain::ChangeRequestReference {
                id: pull["id"].as_str().map(str::to_owned),
                number: pull["number"].as_u64(),
                web_url: pull["url"].as_str().map(str::to_owned),
                author_id: pull["author"]["databaseId"]
                    .as_u64()
                    .map(|id| id.to_string()),
            })
            .collect(),
        commit_authors: AccountSet::new(author.clone(), author.is_some()),
        change_request_authors: associated,
        viewers: if account.is_empty() {
            Vec::new()
        } else {
            vec![ViewerRelationships {
                account_id: account.into(),
                commit_authored: commit["author"]["user"]["isViewer"].as_bool(),
                change_request_authored: if pr_match {
                    Some(true)
                } else if commit["associatedPullRequests"]["pageInfo"]["hasNextPage"] == false
                    && pulls
                        .iter()
                        .all(|pull| pull["viewerDidAuthor"].is_boolean())
                {
                    Some(false)
                } else {
                    None
                },
                ..Default::default()
            }]
        },
        ..Default::default()
    }
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
            fields.push_str(&format!("c{index}: object(expression: $sha{index}) {{ ... on Commit {{ author {{ user {{ databaseId isViewer }} }} associatedPullRequests(first: 100) {{ nodes {{ id number url author {{ ... on User {{ databaseId }} }} viewerDidAuthor }} pageInfo {{ hasNextPage }} }} }} }} "));
            for (pull_index, number) in pulls.iter().enumerate() {
                fields.push_str(&format!(
                    "p{index}_{pull_index}: pullRequest(number: {number}) {{ id number url author {{ ... on User {{ databaseId }} }} viewerDidAuthor }} "
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
            let account = data["viewer"]["databaseId"]
                .as_u64()
                .map(|id| id.to_string())
                .unwrap_or_default();
            let relevance =
                commit_relationships(&repository[format!("c{index}")], &pulls, &account);
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
        run.relationships = results.get(&key).cloned().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests;
