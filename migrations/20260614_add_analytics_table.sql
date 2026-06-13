CREATE TABLE IF NOT EXISTS clicks (
  id BIGSERIAL PRIMARY KEY,
  short_code TEXT NOT NULL,
  ip_addr TEXT NOT NULL,
  referrer TEXT NOT NULL,
  user_agent TEXT NOT NULL,
  clicked_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_clicks_short_code ON clicks(short_code);

CREATE INDEX IF NOT EXISTS idx_clicks_clicked_at ON clicks(clicked_at);
