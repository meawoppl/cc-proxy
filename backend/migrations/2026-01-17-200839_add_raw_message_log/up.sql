-- Table for logging raw-formatted messages for debugging
CREATE TABLE raw_message_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    message_content JSONB NOT NULL,
    message_source VARCHAR(50) NOT NULL,
    render_reason VARCHAR(255),
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Index for querying by creation time (most recent first)
CREATE INDEX idx_raw_message_log_created_at ON raw_message_log(created_at DESC);

-- Index for filtering by session
CREATE INDEX idx_raw_message_log_session_id ON raw_message_log(session_id);
