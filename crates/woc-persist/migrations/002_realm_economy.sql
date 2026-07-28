-- Realm economy blob (mail + auction listings).

CREATE TABLE IF NOT EXISTS realm_economy (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    data JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO realm_economy (id, data)
VALUES (1, '{}'::jsonb)
ON CONFLICT (id) DO NOTHING;
