//! BYOK LLM providers and writing helpers for AuthorKit.

mod brushes;
mod context;
mod error;
mod openai_compat;
mod settings;
mod stream;
pub mod worker;

pub use brushes::{builtin_brushes, Brush};
pub use context::{
    assemble, budget_history, excerpt, AssembledContext, BudgetedHistory, ChatMessage,
    ContextBudget, ContextInput, ContextPiece, ContextRole, GenerationRequest, LlmContext,
    NoteSource, OmitReason, PieceKind, PieceStatus, RequestBudget, Split, SummarySource,
};
pub use error::{redact_secrets, Error, Result};
pub use openai_compat::{ContinueMode, OpenAiCompatClient};
pub use settings::{LlmProfile, LlmSettings, ProviderKind};
pub use worker::{JobIds, JobKind, JobPurpose, LlmEvent, LlmJob, LlmWorker};
