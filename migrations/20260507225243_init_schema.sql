CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE groups (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE group_members (
    group_id BIGINT REFERENCES groups(id) ON DELETE CASCADE,
    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    joined_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (group_id, user_id)
);

CREATE TABLE messages (
    id BIGSERIAL PRIMARY KEY,
    sender_id BIGINT REFERENCES users(id) ON DELETE SET NULL,

    -- Using fields for routing:
    -- If it's a private message, target_user_id will be filled and group will be NULL
    -- If it's a group message, target_group_id will be filled
    target_user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    target_group_id BIGINT REFERENCES groups(id) ON DELETE CASCADE,
    is_broadcast BOOLEAN DEFAULT FALSE,

    content_payload BYTEA NOT NULL,

    created_at BIGINT NOT NULL
);

CREATE INDEX idx_messages_target_user ON messages(target_user_id);
CREATE INDEX idx_messages_target_group ON messages(target_group_id);
CREATE INDEX idx_messages_created_at ON messages(created_at);