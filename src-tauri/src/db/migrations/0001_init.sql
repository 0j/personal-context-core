PRAGMA foreign_keys = ON;

-- Conversations
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY,
    title       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations (updated_at DESC);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    model_used      TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation_id ON messages (conversation_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages (created_at);

-- Artifacts (files, code blocks, etc. attached to messages)
CREATE TABLE IF NOT EXISTS artifacts (
    id              TEXT PRIMARY KEY,
    message_id      TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL,
    content         TEXT NOT NULL,
    mime_type       TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_message_id ON artifacts (message_id);

-- Chunks (text chunks for retrieval)
CREATE TABLE IF NOT EXISTS chunks (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL,
    source_type TEXT NOT NULL,
    content     TEXT NOT NULL,
    tokens      INTEGER,
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_source_id ON chunks (source_id);
CREATE INDEX IF NOT EXISTS idx_chunks_source_type ON chunks (source_type);

-- Embeddings (vector representations of chunks)
CREATE TABLE IF NOT EXISTS embeddings (
    id         TEXT PRIMARY KEY,
    chunk_id   TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    model      TEXT NOT NULL,
    vector     BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_embeddings_chunk_id ON embeddings (chunk_id);

-- Entities (people, places, projects, concepts)
CREATE TABLE IF NOT EXISTS entities (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    name         TEXT NOT NULL,
    aliases      TEXT,
    status       TEXT NOT NULL DEFAULT 'active',
    confidence   REAL NOT NULL DEFAULT 1.0,
    source_id    TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entities_kind ON entities (kind);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities (name);
CREATE INDEX IF NOT EXISTS idx_entities_status ON entities (status);

-- Relationships (edges between entities)
CREATE TABLE IF NOT EXISTS relationships (
    id            TEXT PRIMARY KEY,
    from_entity   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity     TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    weight        REAL NOT NULL DEFAULT 1.0,
    created_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_relationships_from_entity ON relationships (from_entity);
CREATE INDEX IF NOT EXISTS idx_relationships_to_entity ON relationships (to_entity);
CREATE INDEX IF NOT EXISTS idx_relationships_relation_type ON relationships (relation_type);

-- Memories (confirmed facts / autobiographical knowledge)
CREATE TABLE IF NOT EXISTS memories (
    id             TEXT PRIMARY KEY,
    kind           TEXT NOT NULL,
    scope          TEXT NOT NULL,
    sensitivity    TEXT NOT NULL DEFAULT 'internal',
    content        TEXT NOT NULL,
    salience_score REAL NOT NULL DEFAULT 0.5,
    source_kind    TEXT NOT NULL,
    source_id      TEXT,
    entity_id      TEXT REFERENCES entities(id) ON DELETE SET NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    expires_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories (kind);
CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories (scope);
CREATE INDEX IF NOT EXISTS idx_memories_salience ON memories (salience_score DESC);
CREATE INDEX IF NOT EXISTS idx_memories_entity_id ON memories (entity_id);

-- Memory links (many-to-many between memories)
CREATE TABLE IF NOT EXISTS memory_links (
    id          TEXT PRIMARY KEY,
    from_memory TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    to_memory   TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    link_type   TEXT NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_links_from ON memory_links (from_memory);
CREATE INDEX IF NOT EXISTS idx_memory_links_to ON memory_links (to_memory);

-- Memory candidates (unreviewed / pending memories)
CREATE TABLE IF NOT EXISTS memory_candidates (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    content      TEXT NOT NULL,
    source_id    TEXT,
    confidence   REAL NOT NULL DEFAULT 0.5,
    status       TEXT NOT NULL DEFAULT 'pending',
    reviewed_at  TEXT,
    created_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_candidates_status ON memory_candidates (status);
CREATE INDEX IF NOT EXISTS idx_memory_candidates_kind ON memory_candidates (kind);

-- Route decisions (audit log of model routing)
CREATE TABLE IF NOT EXISTS route_decisions (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    message_id      TEXT REFERENCES messages(id) ON DELETE CASCADE,
    model_chosen    TEXT NOT NULL,
    reason          TEXT NOT NULL,
    privacy_level   TEXT NOT NULL,
    score           REAL,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_route_decisions_conversation_id ON route_decisions (conversation_id);
CREATE INDEX IF NOT EXISTS idx_route_decisions_model_chosen ON route_decisions (model_chosen);
CREATE INDEX IF NOT EXISTS idx_route_decisions_created_at ON route_decisions (created_at DESC);

-- Policies (user-defined routing / privacy rules)
CREATE TABLE IF NOT EXISTS policies (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    rule_json   TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_policies_enabled ON policies (enabled);

-- Events (general audit / telemetry log)
CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    payload     TEXT,
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_kind ON events (kind);
CREATE INDEX IF NOT EXISTS idx_events_created_at ON events (created_at DESC);
