-- Add migration script here
CREATE TABLE IF NOT EXISTS config (
    id INTEGER PRIMARY KEY NOT NULL,
    invite_qr TEXT NOT NULL,
    genesis_qr TEXT NOT NULL,
    tester_group INTEGER NOT NULL,
    reviewee_group INTEGER NOT NULL,
    genesis_group INTEGER NOT NULL,
    serial INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_to_chat_type (
    chat_id INTEGER PRIMARY KEY NOT NULL,
    chat_type INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chats (
    submit_chat_id INTEGER PRIMARY KEY NOT NULL,
    review_chat_id INTEGER,
    review_helper INTEGER,
    submit_helper INTEGER,
    publisher INTEGER,
    testers TEXT,
    app_info INTEGER
);

CREATE INDEX IF NOT EXISTS review_chat_id ON chats (review_chat_id);

CREATE TABLE IF NOT EXISTS users (
    contact_id INTEGER PRIMARY KEY NOT NULL,
    tester BOOLEAN NOT NULL,
    publisher BOOLEAN NOT NULL,
    genesis BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS app_infos (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    author_name TEXT NOT NULL,
    author_email TEXT NOT NULL,
    source_code_url TEXT,
    image TEXT,
    description TEXT,
    xdc_blob_dir TEXT,
    version TEXT,
    originator INTEGER NOT NULL,
    active BOOLEAN NOT NULL,
    serial INTEGER NOT NULL
);