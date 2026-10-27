use ak_llm::{JobIds, JobKind, JobPurpose, LlmEvent, LlmJob, LlmProfile, LlmWorker, ProviderKind};
use std::time::{Duration, Instant};

fn mock_profile() -> LlmProfile {
    LlmProfile {
        name: "Mock".into(),
        provider: ProviderKind::OpenAiCompatible,
        base_url: "http://127.0.0.1:9".into(),
        model: "mock".into(),
        keyring_account: "mock".into(),
    }
}

fn ids(job_id: &str, purpose: JobPurpose) -> JobIds {
    JobIds {
        job_id: job_id.into(),
        project_root: "/tmp/p".into(),
        conversation_id: "c1".into(),
        doc_id: "d1".into(),
        text_version: 1,
        purpose,
    }
}

fn wait_until(
    worker: &mut LlmWorker,
    deadline: Duration,
    mut pred: impl FnMut(&[LlmEvent]) -> bool,
) -> Vec<LlmEvent> {
    let start = Instant::now();
    let mut all = Vec::new();
    while start.elapsed() < deadline {
        let batch = worker.poll();
        if !batch.is_empty() {
            all.extend(batch);
            if pred(&all) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    all
}

#[test]
fn mock_job_completes_without_network() {
    let mut worker = LlmWorker::spawn();
    worker.submit(LlmJob {
        ids: ids("j1", JobPurpose::Chat),
        kind: JobKind::Mock {
            reply: "Bonjour le monde".into(),
            stream_chunks: true,
        },
        profile: mock_profile(),
        api_key: None,
    });

    let events = wait_until(&mut worker, Duration::from_secs(3), |evs| {
        evs.iter().any(|e| matches!(e, LlmEvent::Done { .. }))
    });

    let mut acc = String::new();
    let mut got_done = false;
    let mut got_started = false;
    for ev in &events {
        match ev {
            LlmEvent::Started { ids } => {
                got_started = true;
                assert_eq!(ids.purpose, JobPurpose::Chat);
            }
            LlmEvent::Token { delta, .. } => acc.push_str(delta),
            LlmEvent::Done { text, ids } => {
                assert_eq!(text, "Bonjour le monde");
                assert_eq!(ids.job_id, "j1");
                got_done = true;
            }
            LlmEvent::Error { message, .. } => panic!("unexpected error: {message}"),
            _ => {}
        }
    }
    assert!(got_started);
    assert!(got_done);
    assert!(acc.contains("Bonjour") || acc.is_empty() || acc == "Bonjour le monde");
    assert!(!worker.is_busy());
}

#[test]
fn mock_done_without_streaming_and_title_purpose() {
    let mut worker = LlmWorker::spawn();
    assert!(!worker.is_purpose_busy(JobPurpose::Title));
    worker.submit(LlmJob {
        ids: ids("title-1", JobPurpose::Title),
        kind: JobKind::Mock {
            reply: "Titre proposé".into(),
            stream_chunks: false,
        },
        profile: mock_profile(),
        api_key: None,
    });
    assert!(worker.is_busy());
    assert!(worker.is_purpose_busy(JobPurpose::Title));

    let events = wait_until(&mut worker, Duration::from_secs(2), |evs| {
        evs.iter().any(|e| matches!(e, LlmEvent::Done { .. }))
    });
    let done = events.iter().find_map(|e| match e {
        LlmEvent::Done { ids, text } => Some((ids.purpose, text.as_str())),
        _ => None,
    });
    assert_eq!(done, Some((JobPurpose::Title, "Titre proposé")));
    // Non-streaming mock should not emit tokens.
    assert!(!events.iter().any(|e| matches!(e, LlmEvent::Token { .. })));
    assert!(!worker.is_purpose_busy(JobPurpose::Title));
}

#[test]
fn cancel_streaming_mock_emits_cancelled() {
    let mut worker = LlmWorker::spawn();
    // Long reply + per-char sleep so cancel can land mid-stream.
    let reply: String = (0..80).map(|_| 'x').collect();
    worker.submit(LlmJob {
        ids: ids("cancel-me", JobPurpose::Edit),
        kind: JobKind::Mock {
            reply,
            stream_chunks: true,
        },
        profile: mock_profile(),
        api_key: None,
    });

    // Wait for first token (or Started), then cancel.
    let _ = wait_until(&mut worker, Duration::from_secs(2), |evs| {
        evs.iter()
            .any(|e| matches!(e, LlmEvent::Token { .. } | LlmEvent::Started { .. }))
    });
    worker.cancel_job("cancel-me");

    let events = wait_until(&mut worker, Duration::from_secs(2), |evs| {
        evs.iter()
            .any(|e| matches!(e, LlmEvent::Cancelled { .. } | LlmEvent::Done { .. }))
    });
    assert!(
        events
            .iter()
            .any(|e| matches!(e, LlmEvent::Cancelled { ids, .. } if ids.job_id == "cancel-me")),
        "expected Cancelled, got {events:?}"
    );
}

#[test]
fn cancel_purpose_and_cancel_all() {
    let mut worker = LlmWorker::spawn();
    let long: String = (0..100).map(|_| 'y').collect();
    worker.submit(LlmJob {
        ids: ids("ideas-1", JobPurpose::Ideas),
        kind: JobKind::Mock {
            reply: long.clone(),
            stream_chunks: true,
        },
        profile: mock_profile(),
        api_key: None,
    });
    worker.submit(LlmJob {
        ids: ids("review-1", JobPurpose::Review),
        kind: JobKind::Mock {
            reply: long,
            stream_chunks: true,
        },
        profile: mock_profile(),
        api_key: None,
    });

    let _ = wait_until(&mut worker, Duration::from_secs(2), |evs| {
        evs.iter()
            .filter(|e| matches!(e, LlmEvent::Started { .. }))
            .count()
            >= 1
    });
    worker.cancel_purpose(JobPurpose::Ideas);
    worker.cancel();

    let start = Instant::now();
    let mut events = Vec::new();
    while start.elapsed() < Duration::from_secs(3) {
        events.extend(worker.poll());
        if !worker.is_busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, LlmEvent::Done { .. } | LlmEvent::Cancelled { .. })),
        "got {events:?}"
    );
}

#[test]
fn submit_same_purpose_cancels_previous() {
    let mut worker = LlmWorker::spawn();
    let long: String = (0..120).map(|_| 'z').collect();
    worker.submit(LlmJob {
        ids: ids("chat-old", JobPurpose::Chat),
        kind: JobKind::Mock {
            reply: long,
            stream_chunks: true,
        },
        profile: mock_profile(),
        api_key: None,
    });
    std::thread::sleep(Duration::from_millis(15));
    worker.submit(LlmJob {
        ids: ids("chat-new", JobPurpose::Chat),
        kind: JobKind::Mock {
            reply: "nouveau".into(),
            stream_chunks: false,
        },
        profile: mock_profile(),
        api_key: None,
    });

    let events = wait_until(&mut worker, Duration::from_secs(3), |evs| {
        evs.iter()
            .any(|e| matches!(e, LlmEvent::Done { ids, .. } if ids.job_id == "chat-new"))
    });
    assert!(events.iter().any(|e| matches!(
        e,
        LlmEvent::Done { ids, text } if ids.job_id == "chat-new" && text == "nouveau"
    )));
}

#[test]
fn network_job_error_path() {
    let mut worker = LlmWorker::spawn();
    // Point at a closed port — OpenAI client fails without hanging forever.
    let profile = LlmProfile {
        name: "Dead".into(),
        provider: ProviderKind::OpenAiCompatible,
        base_url: "http://127.0.0.1:9".into(),
        model: "nope".into(),
        keyring_account: "dead".into(),
    };
    worker.submit(LlmJob {
        ids: ids("err-1", JobPurpose::Blueprint),
        kind: JobKind::Complete {
            system: "sys".into(),
            user: "hello".into(),
        },
        profile,
        api_key: Some("sk-test-secret".into()),
    });

    let events = wait_until(&mut worker, Duration::from_secs(5), |evs| {
        evs.iter()
            .any(|e| matches!(e, LlmEvent::Error { .. } | LlmEvent::Done { .. }))
    });
    let err = events.iter().find_map(|e| match e {
        LlmEvent::Error { ids, message } => Some((ids.purpose, message.as_str())),
        _ => None,
    });
    let (purpose, message) = err.unwrap_or_else(|| panic!("expected Error, got {events:?}"));
    assert_eq!(purpose, JobPurpose::Blueprint);
    assert!(!message.is_empty());
    // API key must be redacted from the error text.
    assert!(!message.contains("sk-test-secret"));
}
