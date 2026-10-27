//! Background LLM job runner — keeps the UI thread free; streams tokens live.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use crate::context::{ChatMessage, GenerationRequest};
use crate::openai_compat::{ContinueMode, OpenAiCompatClient};
use crate::settings::LlmProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobPurpose {
    Chat,
    Edit,
    Ideas,
    Review,
    Blueprint,
    /// Conversation title from the first user turn.
    Title,
}

#[derive(Debug, Clone)]
pub struct JobIds {
    pub job_id: String,
    pub project_root: String,
    pub conversation_id: String,
    pub doc_id: String,
    pub text_version: u64,
    pub purpose: JobPurpose,
}

#[derive(Debug, Clone)]
pub enum JobKind {
    Chat {
        system: String,
        history: Vec<ChatMessage>,
        user: String,
    },
    Continue {
        system: String,
        tail: String,
        mode: ContinueMode,
    },
    Transform {
        system: String,
        instruction: String,
        selection: String,
    },
    Complete {
        system: String,
        user: String,
    },
    /// Test / offline: echo a fixed reply with optional fake streaming.
    Mock {
        reply: String,
        stream_chunks: bool,
    },
}

#[derive(Clone)]
pub struct LlmJob {
    pub ids: JobIds,
    pub kind: JobKind,
    pub profile: LlmProfile,
    pub api_key: Option<String>,
}

impl std::fmt::Debug for LlmJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmJob")
            .field("ids", &self.ids)
            .field("kind", &self.kind)
            .field("profile", &self.profile)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum LlmEvent {
    Started { ids: JobIds },
    Token { job_id: String, delta: String },
    Done { ids: JobIds, text: String },
    Error { ids: JobIds, message: String },
    Cancelled { ids: JobIds, partial: String },
}

enum WorkerCmd {
    Run(Box<LlmJob>, Arc<AtomicBool>),
    Shutdown,
}

/// Handle to submit jobs and poll events.
pub struct LlmWorker {
    cmd_tx: Sender<WorkerCmd>,
    event_rx: Receiver<LlmEvent>,
    /// Cancel flags keyed by job_id.
    active: HashMap<String, (JobPurpose, Arc<AtomicBool>)>,
}

impl LlmWorker {
    pub fn spawn() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCmd>();
        let (event_tx, event_rx) = mpsc::channel::<LlmEvent>();
        thread::Builder::new()
            .name("ak-llm-worker".into())
            .spawn(move || worker_loop(cmd_rx, event_tx))
            .expect("spawn llm worker");
        Self {
            cmd_tx,
            event_rx,
            active: HashMap::new(),
        }
    }

    /// Submit a job. Cancels any previous job with the same purpose (so a new
    /// chat turn doesn't kill an in-flight brush, and vice versa).
    pub fn submit(&mut self, job: LlmJob) {
        let purpose = job.ids.purpose;
        let to_cancel: Vec<String> = self
            .active
            .iter()
            .filter(|(_, (p, _))| *p == purpose)
            .map(|(id, _)| id.clone())
            .collect();
        for id in to_cancel {
            if let Some((_, flag)) = self.active.get(&id) {
                flag.store(true, Ordering::SeqCst);
            }
            self.active.remove(&id);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.active
            .insert(job.ids.job_id.clone(), (purpose, cancel.clone()));
        let _ = self.cmd_tx.send(WorkerCmd::Run(Box::new(job), cancel));
    }

    pub fn cancel_job(&mut self, job_id: &str) {
        if let Some((_, flag)) = self.active.get(job_id) {
            flag.store(true, Ordering::SeqCst);
        }
    }

    pub fn cancel_purpose(&mut self, purpose: JobPurpose) {
        let ids: Vec<_> = self
            .active
            .iter()
            .filter(|(_, (p, _))| *p == purpose)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.cancel_job(&id);
        }
    }

    pub fn cancel(&mut self) {
        for (_, flag) in self.active.values() {
            flag.store(true, Ordering::SeqCst);
        }
    }

    pub fn is_busy(&self) -> bool {
        !self.active.is_empty()
    }

    pub fn is_purpose_busy(&self, purpose: JobPurpose) -> bool {
        self.active.values().any(|(p, _)| *p == purpose)
    }

    /// Drain available events (non-blocking).
    pub fn poll(&mut self) -> Vec<LlmEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            match &ev {
                LlmEvent::Done { ids, .. }
                | LlmEvent::Error { ids, .. }
                | LlmEvent::Cancelled { ids, .. } => {
                    self.active.remove(&ids.job_id);
                }
                _ => {}
            }
            out.push(ev);
        }
        out
    }
}

impl Drop for LlmWorker {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(WorkerCmd::Shutdown);
    }
}

fn worker_loop(cmd_rx: Receiver<WorkerCmd>, event_tx: Sender<LlmEvent>) {
    // Process jobs on dedicated threads so chat + brush can overlap.
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            WorkerCmd::Shutdown => break,
            WorkerCmd::Run(job, cancel) => {
                let job = *job;
                let tx = event_tx.clone();
                thread::Builder::new()
                    .name(format!(
                        "ak-llm-{}",
                        &job.ids.job_id[..8.min(job.ids.job_id.len())]
                    ))
                    .spawn(move || run_one(job, cancel, tx))
                    .expect("spawn llm job");
            }
        }
    }
}

fn run_one(job: LlmJob, cancel: Arc<AtomicBool>, event_tx: Sender<LlmEvent>) {
    let _ = event_tx.send(LlmEvent::Started {
        ids: job.ids.clone(),
    });
    if cancel.load(Ordering::SeqCst) {
        let _ = event_tx.send(LlmEvent::Cancelled {
            ids: job.ids,
            partial: String::new(),
        });
        return;
    }
    let result = run_job(&job, &cancel, &event_tx);
    if cancel.load(Ordering::SeqCst) {
        let partial = result.unwrap_or_default();
        let _ = event_tx.send(LlmEvent::Cancelled {
            ids: job.ids,
            partial,
        });
    } else {
        match result {
            Ok(text) => {
                let _ = event_tx.send(LlmEvent::Done { ids: job.ids, text });
            }
            Err(message) => {
                let message = crate::error::redact_secrets(&message, job.api_key.as_deref());
                let _ = event_tx.send(LlmEvent::Error {
                    ids: job.ids,
                    message,
                });
            }
        }
    }
}

fn run_job(
    job: &LlmJob,
    cancel: &AtomicBool,
    event_tx: &Sender<LlmEvent>,
) -> std::result::Result<String, String> {
    match &job.kind {
        JobKind::Mock {
            reply,
            stream_chunks,
        } => {
            if *stream_chunks {
                let mut acc = String::new();
                for ch in reply.chars() {
                    if cancel.load(Ordering::SeqCst) {
                        return Ok(acc);
                    }
                    let delta = ch.to_string();
                    acc.push_str(&delta);
                    let _ = event_tx.send(LlmEvent::Token {
                        job_id: job.ids.job_id.clone(),
                        delta,
                    });
                    thread::sleep(std::time::Duration::from_millis(4));
                }
                Ok(acc)
            } else {
                Ok(reply.clone())
            }
        }
        other => {
            let client = OpenAiCompatClient::new(job.profile.clone(), job.api_key.clone());
            let req = match other {
                JobKind::Chat {
                    system,
                    history,
                    user,
                } => {
                    let mut messages = history.clone();
                    messages.push(ChatMessage {
                        role: "user".into(),
                        content: user.clone(),
                    });
                    GenerationRequest {
                        system: system.clone(),
                        messages,
                    }
                }
                JobKind::Continue { system, tail, mode } => {
                    let prompt = continue_prompt(*mode);
                    GenerationRequest {
                        system: system.clone(),
                        messages: vec![ChatMessage {
                            role: "user".into(),
                            content: format!("{prompt}\n\n---\nFin du draft:\n\n{tail}"),
                        }],
                    }
                }
                JobKind::Transform {
                    system,
                    instruction,
                    selection,
                } => GenerationRequest {
                    system: system.clone(),
                    messages: vec![ChatMessage {
                        role: "user".into(),
                        content: format!(
                            "{instruction}\n\n---\nTexte à transformer:\n\n{selection}"
                        ),
                    }],
                },
                JobKind::Complete { system, user } => GenerationRequest {
                    system: system.clone(),
                    messages: vec![ChatMessage {
                        role: "user".into(),
                        content: user.clone(),
                    }],
                },
                JobKind::Mock { .. } => unreachable!(),
            };

            let job_id = job.ids.job_id.clone();
            let tx = event_tx.clone();
            client
                .stream_complete(&req, cancel, |delta| {
                    let _ = tx.send(LlmEvent::Token {
                        job_id: job_id.clone(),
                        delta: delta.to_string(),
                    });
                })
                .map_err(|e| e.to_string())
        }
    }
}

fn continue_prompt(mode: ContinueMode) -> &'static str {
    match mode {
        ContinueMode::Sentence => {
            "Continue en écrivant UNE phrase suivante, cohérente avec le draft. \
             Renvoie uniquement cette phrase, sans guillemets ni commentaire."
        }
        ContinueMode::Paragraph => {
            "Continue en écrivant UN paragraphe suivant. \
             Renvoie uniquement ce paragraphe, sans commentaire."
        }
        ContinueMode::Continue => {
            "Continue l'écriture de façon naturelle (quelques phrases à un court paragraphe). \
             Renvoie uniquement le texte à ajouter, sans commentaire."
        }
        ContinueMode::Generate => {
            "Génère la suite demandée. Renvoie uniquement le texte à insérer."
        }
    }
}
