//! Local scripted HTTP server for provider contract tests; no real tokens or services.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) fn relevance_repository() -> crate::domain::Repository {
    crate::domain::Repository {
        id: "3".into(),
        owner: "team".into(),
        name: "app".into(),
        description: None,
        visibility: crate::domain::RepositoryVisibility::Private,
        web_url: "https://provider.example/team/app".into(),
    }
}

pub(super) fn relevance_run(sha: &str) -> crate::domain::WorkflowRun {
    use crate::domain::*;
    WorkflowRun {
        relationships: Relationships::default(),
        id: "run".into(),
        workflow_id: "build".into(),
        run_number: 1,
        attempt: 1,
        title: "Build".into(),
        lifecycle: RunLifecycle::Completed,
        outcome: RunOutcome::Failure,
        branch: Some("main".into()),
        commit_sha: sha.into(),
        actor: Some("me".into()),
        trigger: "push".into(),
        created_at: String::new(),
        started_at: None,
        updated_at: String::new(),
        web_url: String::new(),
        provider_status: "failed".into(),
        provider_conclusion: Some("failed".into()),
    }
}

pub(super) struct Exchange {
    pub method: String,
    pub request_body: Option<serde_json::Value>,
    pub path: String,
    pub authorization: String,
    pub status: u16,
    pub headers: String,
    pub body: String,
}

impl Exchange {
    pub fn graphql(
        path: &str,
        authorization: &str,
        variables: serde_json::Value,
        query_fragments: &[&str],
        response: serde_json::Value,
    ) -> Self {
        let mut exchange = Self::json(path, authorization, response);
        // Assert the emitted variables and required selections independently
        // of production query builders and insignificant query whitespace.
        exchange.request_body =
            Some(serde_json::json!({"variables": variables, "query_fragments": query_fragments}));
        exchange.method = "POST".into();
        exchange
    }

    pub fn denied(path: &str, authorization: &str) -> Self {
        let mut exchange = Self::json(path, authorization, serde_json::json!({}));
        exchange.status = 403;
        exchange
    }

    pub fn json(path: &str, authorization: &str, body: serde_json::Value) -> Self {
        Self {
            method: "GET".into(),
            request_body: None,
            path: path.into(),
            authorization: authorization.into(),
            status: 200,
            headers: "Content-Type: application/json\r\n".into(),
            body: body.to_string(),
        }
    }
}

pub(super) struct MockApi {
    pub url: String,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<usize>>,
    expected: usize,
}

impl MockApi {
    pub fn start(exchanges: Vec<Exchange>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider fixture");
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let expected = exchanges.len();
        let worker = std::thread::spawn(move || {
            let mut handled = 0;
            for exchange in exchanges {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    if stop.load(Ordering::Relaxed) {
                        return handled;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "provider request timed out: {}",
                                exchange.path
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept fixture request: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 16_384);
                }
                let request = String::from_utf8(request).unwrap();
                assert_eq!(
                    request.lines().next().unwrap(),
                    format!("{} {} HTTP/1.1", exchange.method, exchange.path)
                );
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains(&exchange.authorization.to_ascii_lowercase())
                );
                if let Some(expected) = &exchange.request_body {
                    let length = request
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .expect("JSON content length");
                    assert!(length < 1_048_576);
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).unwrap();
                    let actual: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    if let Some(fragments) = expected["query_fragments"].as_array() {
                        assert_eq!(actual["variables"], expected["variables"]);
                        let query = actual["query"].as_str().expect("GraphQL query");
                        for fragment in fragments {
                            assert!(
                                query.contains(fragment.as_str().unwrap()),
                                "missing GraphQL selection: {fragment}"
                            );
                        }
                    } else {
                        assert_eq!(&actual, expected);
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 {} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                    exchange.status,
                    exchange.body.len(),
                    exchange.headers,
                    exchange.body
                )
                .unwrap();
                handled += 1;
            }
            handled
        });
        Self {
            url,
            stopped,
            worker: Some(worker),
            expected,
        }
    }

    pub fn finish(mut self) {
        assert_eq!(
            self.worker
                .take()
                .unwrap()
                .join()
                .expect("provider fixture"),
            self.expected
        );
    }
}

impl Drop for MockApi {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
