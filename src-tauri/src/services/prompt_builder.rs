use crate::models::prompt::PromptContext;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build the full system prompt string from a `PromptContext`.
    pub fn build_system_prompt(&self, ctx: &PromptContext) -> String {
        let mut parts: Vec<String> = Vec::new();

        // ── Core principle ────────────────────────────────────────────────
        parts.push(
            "You are Personal Context Core, a local-first personal AI assistant.\n\
             Principle: Local by default, cloud by exception, user visible always.\n\
             You have access to the user's personal memory and context stored on their device.\n\
             Respect privacy. Never speculate about sensitive data. Be concise and honest."
                .to_string(),
        );

        // ── Active policies ───────────────────────────────────────────────
        let enabled_policies: Vec<_> = ctx.active_policies.iter().filter(|p| p.enabled).collect();
        if !enabled_policies.is_empty() {
            let mut section = "## Active Policies\n".to_string();
            for policy in &enabled_policies {
                section.push_str(&format!(
                    "- **{}**: {}\n",
                    policy.name,
                    policy.description.as_deref().unwrap_or("(no description)")
                ));
            }
            parts.push(section);
        }

        // ── Known entities ────────────────────────────────────────────────
        if !ctx.entity_names.is_empty() {
            let mut section = "## Known Entities\n".to_string();
            for name in &ctx.entity_names {
                section.push_str(&format!("- {}\n", name));
            }
            parts.push(section);
        }

        // ── Retrieved memories ────────────────────────────────────────────
        if !ctx.memories.is_empty() {
            let mut section = "## Relevant Memories\n".to_string();
            for mem in &ctx.memories {
                section.push_str(&format!(
                    "- [{}] (salience {:.2}) {}\n",
                    mem.kind.as_str(),
                    mem.salience_score,
                    mem.content,
                ));
            }
            parts.push(section);
        }

        // ── Retrieved chunks ──────────────────────────────────────────────
        if !ctx.chunks.is_empty() {
            let mut section = "## Relevant Context\n".to_string();
            for chunk in &ctx.chunks {
                section.push_str(&format!(
                    "### Source: {} (score {:.2})\n{}\n",
                    chunk.source_type, chunk.score, chunk.content
                ));
            }
            parts.push(section);
        }

        parts.join("\n\n")
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
