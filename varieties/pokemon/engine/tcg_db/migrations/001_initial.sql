PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sets (
  set_id        INTEGER PRIMARY KEY,
  code          TEXT NOT NULL UNIQUE,
  name          TEXT NOT NULL,
  release_date  TEXT,
  era           TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cards (
  card_def_id    TEXT PRIMARY KEY,
  set_id         INTEGER NOT NULL REFERENCES sets(set_id),
  number         TEXT NOT NULL,
  name           TEXT NOT NULL,
  supertype      TEXT NOT NULL,
  subtypes_json  TEXT NOT NULL,
  tags_json      TEXT NOT NULL,
  stage          TEXT,
  evolves_from   TEXT,
  hp             INTEGER,
  types_json     TEXT,
  weakness_json  TEXT,
  resist_json    TEXT,
  retreat_cost   INTEGER,
  trainer_kind   TEXT,
  energy_kind    TEXT,
  script_kind    TEXT NOT NULL,
  script_payload TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cards_set_number ON cards(set_id, number);
CREATE INDEX IF NOT EXISTS idx_cards_name ON cards(name);

CREATE VIRTUAL TABLE IF NOT EXISTS cards_fts
USING fts5(name, content='cards', content_rowid='rowid');

CREATE TABLE IF NOT EXISTS attacks (
  attack_id    INTEGER PRIMARY KEY,
  card_def_id  TEXT NOT NULL REFERENCES cards(card_def_id),
  idx          INTEGER NOT NULL,
  name         TEXT NOT NULL,
  cost_json    TEXT NOT NULL,
  damage_expr  TEXT NOT NULL,
  effect_ast   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS powers (
  power_id     INTEGER PRIMARY KEY,
  card_def_id  TEXT NOT NULL REFERENCES cards(card_def_id),
  idx          INTEGER NOT NULL,
  kind         TEXT NOT NULL,
  name         TEXT NOT NULL,
  effect_ast   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS decks (
  deck_id     TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  format      TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS deck_cards (
  deck_id     TEXT NOT NULL REFERENCES decks(deck_id),
  card_def_id TEXT NOT NULL REFERENCES cards(card_def_id),
  count       INTEGER NOT NULL,
  PRIMARY KEY (deck_id, card_def_id)
);

CREATE TABLE IF NOT EXISTS games (
  game_id     TEXT PRIMARY KEY,
  created_at  TEXT NOT NULL,
  ruleset_ver TEXT NOT NULL,
  rng_seed    TEXT NOT NULL,
  deck0_id    TEXT NOT NULL REFERENCES decks(deck_id),
  deck1_id    TEXT NOT NULL REFERENCES decks(deck_id),
  winner      INTEGER,
  result_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS game_log (
  game_id     TEXT NOT NULL REFERENCES games(game_id),
  seq         INTEGER NOT NULL,
  record_json TEXT NOT NULL,
  PRIMARY KEY (game_id, seq)
);

CREATE TABLE IF NOT EXISTS game_state (
  game_id     TEXT NOT NULL REFERENCES games(game_id),
  state_json  TEXT NOT NULL,
  PRIMARY KEY (game_id)
);
