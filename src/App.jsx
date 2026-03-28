import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// RouteBadge
// route_decision.model_chosen  → ModelOption enum → "cloud_fast" | "cloud_reasoning" | "local_general"
// route_decision.privacy_level → PrivacyLevel enum → "public" | "sensitive" | "restricted"
// assistant_message.model_used → same registry slot string
// ---------------------------------------------------------------------------
function RouteBadge({ routeDecision, assistantMessage }) {
  if (!routeDecision) return null;

  const modelName =
    assistantMessage?.model_used ||
    routeDecision?.model_chosen ||
    "unknown";

  const privacy = routeDecision?.privacy_level || "unknown";

  return (
    <div style={styles.routeBadge}>
      <span style={styles.routeModel}>{modelName}</span>
      <span style={styles.routeDivider}>•</span>
      <span style={styles.routePrivacy}>{privacy}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// MemoryInboxItem
// MemoryCandidate fields: { id, kind, content, confidence, status, ... }
//   kind    → CandidateType snake_case string ("fact", "preference", …)
//   content → JSON string (MemoryCandidateRaw) written by the extraction worker
// ---------------------------------------------------------------------------
function MemoryInboxItem({ item, onReview }) {
  const payload = (() => {
    try {
      return JSON.parse(item.content);
    } catch {
      return {};
    }
  })();

  const title =
    payload?.statement ||
    payload?.summary ||
    item.content ||
    "Untitled candidate";

  return (
    <div style={styles.inboxItem}>
      <div style={styles.inboxItemTop}>
        <div style={styles.inboxType}>{item.kind}</div>
        <div style={styles.inboxConfidence}>
          {(item.confidence ?? 0).toFixed(2)}
        </div>
      </div>

      <div style={styles.inboxTitle}>{title}</div>

      {payload?.summary && payload.summary !== title ? (
        <div style={styles.inboxSummary}>{payload.summary}</div>
      ) : null}

      {payload?.rationale ? (
        <div style={styles.inboxRationale}>{payload.rationale}</div>
      ) : null}

      <div style={styles.inboxActions}>
        <button
          style={{ ...styles.button, ...styles.approveButton }}
          onClick={() => onReview(item.id, "approved")}
        >
          Approve
        </button>
        <button
          style={{ ...styles.button, ...styles.rejectButton }}
          onClick={() => onReview(item.id, "rejected")}
        >
          Reject
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// MessageBubble
// Message fields: { id, conversation_id, role, content, model_used, created_at }
//   role → MessageRole snake_case → "user" | "assistant"
// ---------------------------------------------------------------------------
function MessageBubble({ msg }) {
  const isUser = msg.role === "user";

  return (
    <div
      style={{
        ...styles.messageRow,
        justifyContent: isUser ? "flex-end" : "flex-start",
      }}
    >
      <div
        style={{
          ...styles.messageBubble,
          ...(isUser ? styles.userBubble : styles.assistantBubble),
        }}
      >
        <div style={styles.messageRole}>
          {isUser ? "You" : "Assistant"}
        </div>
        <div style={styles.messageContent}>{msg.content}</div>

        {!isUser ? (
          <div style={styles.assistantMeta}>
            <RouteBadge
              routeDecision={msg.routeDecision}
              assistantMessage={msg}
            />
            <div style={styles.memoryCount}>
              memories injected: {msg.injectedMemoryCount ?? 0}
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------
export default function App() {
  const [conversationId, setConversationId] = useState(null);
  const [messages, setMessages] = useState([]);
  const [pendingCandidates, setPendingCandidates] = useState([]);
  const [input, setInput] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [bootError, setBootError] = useState(null);

  const listRef = useRef(null);

  async function createConversation() {
    // create_conversation(title: Option<String>) → Conversation { id, title, created_at, updated_at }
    const conversation = await invoke("create_conversation", {
      title: "New Conversation",
    });
    setConversationId(conversation.id);
  }

  async function refreshInbox() {
    try {
      // list_memory_candidates() — no arguments
      const items = await invoke("list_memory_candidates");
      setPendingCandidates(items || []);
    } catch (err) {
      console.error("Failed to load memory inbox", err);
    }
  }

  async function handleReview(candidateId, newStatus) {
    try {
      // review_memory_candidate(candidate_id: String, new_status: String) → ()
      // Tauri maps camelCase JS keys to snake_case Rust params:
      //   candidateId → candidate_id   newStatus → new_status
      // new_status must be a CandidateStatus string: "approved" | "rejected" | "deferred"
      await invoke("review_memory_candidate", { candidateId, newStatus });
      await refreshInbox();
    } catch (err) {
      console.error("Failed to review candidate", err);
    }
  }

  async function handleSend() {
    const text = input.trim();
    if (!text || !conversationId || isSending) return;

    const tempUser = {
      id: `temp-user-${Date.now()}`,
      role: "user",
      content: text,
    };

    setMessages((prev) => [...prev, tempUser]);
    setInput("");
    setIsSending(true);

    try {
      // send_chat_turn(conversation_id: String, user_content: String)
      //   → SendChatTurnResponse { assistant_message, route_decision, model_used }
      // Tauri maps: conversationId → conversation_id, userContent → user_content
      // Note: response has no user_message or injected_memory_count fields.
      const res = await invoke("send_chat_turn", {
        conversationId,
        userContent: text,
      });

      // Keep the optimistic tempUser message already in state — no user_message in response.
      // injected_memory_count is not returned; show 0.
      const assistant = {
        ...res.assistant_message,
        role: "assistant",
        routeDecision: res.route_decision,
        injectedMemoryCount: 0,
      };

      setMessages((prev) => [...prev, assistant]);

      await refreshInbox();
    } catch (err) {
      console.error("Failed to send chat turn", err);
      setMessages((prev) => [
        ...prev,
        {
          id: `temp-error-${Date.now()}`,
          role: "assistant",
          content: `Error: ${String(err)}`,
          routeDecision: null,
          injectedMemoryCount: 0,
        },
      ]);
    } finally {
      setIsSending(false);
    }
  }

  useEffect(() => {
    let mounted = true;

    (async () => {
      try {
        await createConversation();
        await refreshInbox();
      } catch (err) {
        if (mounted) setBootError(String(err));
      }
    })();

    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    if (!listRef.current) return;
    listRef.current.scrollTop = listRef.current.scrollHeight;
  }, [messages]);

  const disabled = !conversationId || isSending;

  return (
    <div style={styles.app}>
      <div style={styles.chatPane}>
        <div style={styles.header}>
          <div>
            <div style={styles.title}>Personal Context Core</div>
            <div style={styles.subtitle}>
              {conversationId
                ? `conversation: ${conversationId}`
                : "starting..."}
            </div>
          </div>
        </div>

        <div ref={listRef} style={styles.messageList}>
          {bootError ? (
            <div style={styles.errorBox}>{bootError}</div>
          ) : messages.length === 0 ? (
            <div style={styles.emptyState}>
              Start typing to test routing, memory injection, and extraction.
            </div>
          ) : (
            messages.map((msg) => <MessageBubble key={msg.id} msg={msg} />)
          )}
        </div>

        <div style={styles.composer}>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Ask something..."
            style={styles.textarea}
            rows={3}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
          />
          <button
            style={{
              ...styles.button,
              ...(disabled ? styles.buttonDisabled : styles.sendButton),
            }}
            onClick={handleSend}
            disabled={disabled}
          >
            {isSending ? "Sending..." : "Send"}
          </button>
        </div>
      </div>

      <div style={styles.sidebar}>
        <div style={styles.sidebarHeader}>
          <div style={styles.sidebarTitle}>Memory Inbox</div>
          <button style={styles.refreshLink} onClick={refreshInbox}>
            Refresh
          </button>
        </div>

        <div style={styles.inboxList}>
          {pendingCandidates.length === 0 ? (
            <div style={styles.emptyInbox}>No pending candidates yet.</div>
          ) : (
            pendingCandidates.map((item) => (
              <MemoryInboxItem
                key={item.id}
                item={item}
                onReview={handleReview}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------
const styles = {
  app: {
    display: "grid",
    gridTemplateColumns: "1fr 360px",
    height: "100vh",
    background: "#0b0f14",
    color: "#e8edf2",
    fontFamily:
      'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  },
  chatPane: {
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
    borderRight: "1px solid #1b2430",
  },
  header: {
    padding: "18px 20px",
    borderBottom: "1px solid #1b2430",
    background: "#0e141b",
  },
  title: {
    fontSize: 20,
    fontWeight: 700,
    letterSpacing: "-0.02em",
  },
  subtitle: {
    marginTop: 4,
    fontSize: 12,
    color: "#8ea0b5",
  },
  messageList: {
    flex: 1,
    overflowY: "auto",
    padding: "24px 20px",
  },
  emptyState: {
    color: "#7f8ea3",
    fontSize: 14,
  },
  errorBox: {
    padding: 16,
    borderRadius: 12,
    background: "#35171b",
    color: "#ffb7c0",
    border: "1px solid #5c2731",
  },
  messageRow: {
    display: "flex",
    marginBottom: 16,
  },
  messageBubble: {
    maxWidth: "78%",
    borderRadius: 16,
    padding: "14px 16px",
    boxShadow: "0 8px 24px rgba(0,0,0,0.22)",
  },
  userBubble: {
    background: "#1c3d5a",
    border: "1px solid #29557b",
  },
  assistantBubble: {
    background: "#131b24",
    border: "1px solid #243140",
  },
  messageRole: {
    fontSize: 12,
    fontWeight: 700,
    color: "#8ea0b5",
    marginBottom: 8,
    textTransform: "uppercase",
    letterSpacing: "0.06em",
  },
  messageContent: {
    whiteSpace: "pre-wrap",
    lineHeight: 1.55,
    fontSize: 14,
  },
  assistantMeta: {
    marginTop: 12,
    display: "flex",
    alignItems: "center",
    gap: 12,
    flexWrap: "wrap",
  },
  routeBadge: {
    display: "inline-flex",
    alignItems: "center",
    gap: 8,
    padding: "6px 10px",
    borderRadius: 999,
    background: "#0f2230",
    border: "1px solid #264257",
    fontSize: 12,
  },
  routeModel: {
    color: "#cde7ff",
    fontWeight: 600,
  },
  routeDivider: {
    color: "#6083a3",
  },
  routePrivacy: {
    color: "#9bc0de",
  },
  memoryCount: {
    fontSize: 12,
    color: "#8ea0b5",
  },
  composer: {
    display: "flex",
    gap: 12,
    padding: 16,
    borderTop: "1px solid #1b2430",
    background: "#0e141b",
  },
  textarea: {
    flex: 1,
    resize: "none",
    borderRadius: 14,
    border: "1px solid #2a3644",
    background: "#0c1117",
    color: "#e8edf2",
    padding: 14,
    fontSize: 14,
    outline: "none",
  },
  button: {
    borderRadius: 12,
    border: "none",
    padding: "0 16px",
    fontWeight: 600,
    cursor: "pointer",
  },
  sendButton: {
    background: "#3b82f6",
    color: "white",
    minWidth: 92,
  },
  buttonDisabled: {
    background: "#2a3340",
    color: "#7b8796",
    cursor: "not-allowed",
    minWidth: 92,
  },
  sidebar: {
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
    background: "#0d1218",
  },
  sidebarHeader: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "18px 16px",
    borderBottom: "1px solid #1b2430",
  },
  sidebarTitle: {
    fontSize: 16,
    fontWeight: 700,
  },
  refreshLink: {
    background: "transparent",
    color: "#8fbef3",
    border: "none",
    cursor: "pointer",
    fontSize: 13,
  },
  inboxList: {
    flex: 1,
    overflowY: "auto",
    padding: 12,
  },
  emptyInbox: {
    color: "#7f8ea3",
    fontSize: 14,
    padding: 8,
  },
  inboxItem: {
    border: "1px solid #23303d",
    background: "#121922",
    borderRadius: 14,
    padding: 12,
    marginBottom: 12,
  },
  inboxItemTop: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 8,
  },
  inboxType: {
    fontSize: 11,
    textTransform: "uppercase",
    letterSpacing: "0.06em",
    color: "#89a0b9",
  },
  inboxConfidence: {
    fontSize: 12,
    color: "#d4e8ff",
  },
  inboxTitle: {
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.4,
  },
  inboxSummary: {
    marginTop: 8,
    fontSize: 13,
    color: "#b7c4d4",
    lineHeight: 1.45,
  },
  inboxRationale: {
    marginTop: 8,
    fontSize: 12,
    color: "#8ea0b5",
    lineHeight: 1.45,
  },
  inboxActions: {
    display: "flex",
    gap: 8,
    marginTop: 12,
  },
  approveButton: {
    background: "#173624",
    color: "#b4f2cb",
    border: "1px solid #25563a",
    padding: "8px 10px",
  },
  rejectButton: {
    background: "#35171b",
    color: "#ffb7c0",
    border: "1px solid #5c2731",
    padding: "8px 10px",
  },
};
