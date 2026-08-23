import { useEffect, useMemo, useRef, useState } from "react";

type Deck = { id: string; name: string; archetype: string; cards: number };
type Card = {
  id: number;
  name: string;
  definition?: string;
  types: string[];
  mana_cost: string;
  colors: string[];
  mana_colors: string[];
  power?: number;
  toughness?: number;
  tapped: boolean;
  can_attack: boolean;
  can_block: boolean;
  summoning_sick: boolean;
};
type Target = { kind: string; id: number; label: string };
type Action = {
  kind: string;
  card?: number;
  label: string;
  pay_life?: boolean;
  targets: Target[];
  attackers: number[];
  mana_color?: string;
};
type Seat = {
  seat: number;
  life: number;
  hand: Card[];
  battlefield: Card[];
  graveyard_count: number;
  library_count: number;
  mana: Record<string, number>;
  lands_played: number;
};
type StackItem = { id: number; card?: Card; controller: number; targets: string[] };
type GameState = {
  schema: string;
  match_id: string;
  revision: number;
  seed: number;
  human_deck: string;
  ai_deck: string;
  opponent_policy: string;
  turn: number;
  step: string;
  active_player: number;
  priority: number;
  decision_player: number;
  awaiting_human: boolean;
  terminal: boolean;
  winner?: number;
  error?: string;
  human: Seat;
  opponent: Seat;
  stack: StackItem[];
  attackers: number[];
  attackers_declared: boolean;
  blockers_declared: boolean;
  action_surface: Action[];
  log: string[];
};

const defaultHuman = "rav_boros_aggro";
const defaultAi = "rav_selesnya_midrange";

export default function App() {
  const [decks, setDecks] = useState<Deck[]>([]);
  const [humanDeck, setHumanDeck] = useState(defaultHuman);
  const [aiDeck, setAiDeck] = useState(defaultAi);
  const [policyVersion, setPolicyVersion] = useState("v7");
  const [seed, setSeed] = useState("73");
  const [state, setState] = useState<GameState>();
  const [error, setError] = useState<string>();
  const [vlmMode, setVlmMode] = useState(false);
  const [selectedAttackers, setSelectedAttackers] = useState<number[]>([]);
  const [targetSelections, setTargetSelections] = useState<Record<number, string>>({});
  const socket = useRef<WebSocket | null>(null);

  useEffect(() => {
    fetch("/api/decks")
      .then((response) => response.json() as Promise<Deck[]>)
      .then((items) => {
        setDecks(items);
        if (items.length > 0 && !items.some((item) => item.id === humanDeck)) setHumanDeck(items[0].id);
        if (items.length > 1 && !items.some((item) => item.id === aiDeck)) setAiDeck(items[1].id);
      })
      .catch((reason: Error) => setError(reason.message));
    return () => socket.current?.close();
  }, []);

  const selectedHuman = decks.find((deck) => deck.id === humanDeck);
  const selectedAi = decks.find((deck) => deck.id === aiDeck);

  function connectAndStart() {
    setError(undefined);
    socket.current?.close();
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
    socket.current = ws;
    ws.onopen = () => {
      ws.send(JSON.stringify({ type: "start", human_deck: humanDeck, ai_deck: aiDeck, policy_version: policyVersion, seed: Number(seed) || 73 }));
    };
    ws.onmessage = (event) => {
      const payload = JSON.parse(event.data) as GameState & { schema: string; error?: string };
      if (payload.schema.endsWith(".error")) setError(payload.error ?? "server error");
      else {
        setState(payload);
        setError(payload.error);
        setSelectedAttackers([]);
        setTargetSelections({});
      }
    };
    ws.onerror = () => setError("Could not connect to the Rust server. Start rav-magic-server first.");
  }

  function send(action: object) {
    if (!socket.current || socket.current.readyState !== WebSocket.OPEN || !state) {
      setError("Start a match first.");
      return;
    }
    socket.current.send(JSON.stringify({ type: "action", match_id: state.match_id, action }));
  }

  function action(actionItem: Action) {
    if (actionItem.kind === "pass_priority") send({ kind: "pass_priority" });
    if (actionItem.kind === "draw") send({ kind: "draw" });
    if (actionItem.kind === "play_land") send({ kind: "play_land", card: actionItem.card, pay_life: actionItem.pay_life ?? false });
    if (actionItem.kind === "add_mana") send({ kind: "add_mana", land: actionItem.card, color: actionItem.mana_color ?? "" });
    if (actionItem.kind === "cast") {
      const targetKey = targetSelections[actionItem.card ?? 0];
      const selectedTarget = actionItem.targets.find((target) => `${target.kind}:${target.id}` === targetKey);
      send({
        kind: "cast",
        card: actionItem.card,
        target: selectedTarget ? { kind: selectedTarget.kind, id: selectedTarget.id } : null,
      });
    }
  }

  const humanPrompt = useMemo(() => {
    if (!state) return "Choose a deck to begin.";
    if (state.terminal) return state.winner === 0 ? "You won." : state.winner === 1 ? `${state.opponent_policy} won.` : "The game ended.";
    if (!state.awaiting_human) return `${state.opponent_policy} is thinking…`;
    if (state.step === "DeclareAttackers" && !state.attackers_declared) return "Declare attackers, then send them into combat.";
    if (state.step === "DeclareBlockers" && !state.blockers_declared) return "Assign blocks or choose no blocks.";
    return state.priority === 0 ? "You have priority." : "Choose the pending human decision.";
  }, [state]);

  return (
    <main className={vlmMode ? "app vlm-mode" : "app"}>
      <header className="topbar">
        <div>
          <div className="eyebrow">CARDBENCH / MAGIC / RAVNICA</div>
          <h1>Local Match Lab</h1>
        </div>
        <div className="top-actions">
          <button className={vlmMode ? "active" : ""} onClick={() => setVlmMode((value) => !value)}>
            {vlmMode ? "Normal view" : "VLM view"}
          </button>
          <button onClick={() => state && navigator.clipboard?.writeText(JSON.stringify(state, null, 2))} disabled={!state}>
            Copy observation JSON
          </button>
        </div>
      </header>

      {!state && (
        <section className="start-panel">
          <div className="start-copy">
            <div className="eyebrow">REAL ENGINE / LOCAL ONLY</div>
            <h2>Play against a policy.</h2>
            <p>Every card, priority pass, combat declaration, and rules check is coming from the Ravnica engine.</p>
          </div>
          <div className="setup-grid">
            <label>Human deck<select value={humanDeck} onChange={(event) => setHumanDeck(event.target.value)}>{decks.map((deck) => <option value={deck.id} key={deck.id}>{deck.name}</option>)}</select></label>
            <label>Opponent deck<select value={aiDeck} onChange={(event) => setAiDeck(event.target.value)}>{decks.map((deck) => <option value={deck.id} key={deck.id}>{deck.name}</option>)}</select></label>
            <label>Opponent policy<select value={policyVersion} onChange={(event) => setPolicyVersion(event.target.value)}>{["v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8"].map((version) => <option value={version} key={version}>{version}</option>)}</select></label>
            <label>Shuffle seed<input value={seed} onChange={(event) => setSeed(event.target.value)} inputMode="numeric" /></label>
            <button className="primary start-button" onClick={connectAndStart}>Start match</button>
          </div>
          {selectedHuman && selectedAi && <div className="deck-note">{selectedHuman.name} · {selectedHuman.archetype} vs {selectedAi.name} · {selectedAi.archetype} · {policyVersion} · 60 cards each</div>}
        </section>
      )}

      {error && <div className="error-banner" role="alert">{error}</div>}
      {state && (
        <section className="match-shell" aria-label="Magic game board" data-testid="game-board">
          <div className="match-meta">
            <span className="status-dot" /> <strong>{humanPrompt}</strong>
            <span>Turn {state.turn}</span><span>{state.step}</span><span>Revision {state.revision}</span>
            <span className="match-id">{state.match_id} · seed {state.seed}</span>
          </div>
          <div className="board">
            <SeatPanel seat={state.opponent} opponent label={`${selectedAi?.name ?? state.ai_deck} · ${state.opponent_policy}`} />
            <div className="center-rail">
              <div className="life-strip"><span>{state.opponent_policy} {state.opponent.life}</span><span>Turn {state.turn}</span><span>You {state.human.life}</span></div>
              <div className="stack-area"><div className="zone-title">STACK</div>{state.stack.length === 0 ? <div className="empty-zone">empty</div> : state.stack.map((item) => <div className="stack-item" key={item.id}><span>{item.card?.name ?? "Ability"}</span><small>seat {item.controller} · {item.targets.join(" · ")}</small></div>)}</div>
              <div className="combat-callout">{state.attackers.length ? `${state.attackers.length} attacker(s) declared` : "No attackers declared"}</div>
            </div>
            <SeatPanel seat={state.human} label={`${selectedHuman?.name ?? state.human_deck} · HUMAN`} />
          </div>
          <div className="controls">
            <div className="controls-header"><div><div className="eyebrow">ACTION SURFACE</div><h2>{humanPrompt}</h2></div><span className="scope-chip">public observation · rev {state.revision}</span></div>
            {state.step === "DeclareAttackers" && !state.attackers_declared && state.awaiting_human && <CombatSelector cards={state.human.battlefield.filter((card) => state.action_surface[0]?.attackers.includes(card.id))} selected={selectedAttackers} setSelected={setSelectedAttackers} onSubmit={() => send({ kind: "declare_attackers", attackers: selectedAttackers })} label="Attack with selected" />}
            {state.step === "DeclareBlockers" && !state.blockers_declared && state.awaiting_human && <button className="primary" onClick={() => send({ kind: "declare_blockers", assignments: [] })}>Take no blocks</button>}
            <div className="action-grid">{state.action_surface.filter((item) => !["declare_attackers", "declare_blockers", "decision"].includes(item.kind)).map((item, index) => <ActionButton item={item} key={`${item.kind}-${item.card ?? "none"}-${item.mana_color ?? ""}-${item.pay_life ?? ""}-${index}`} targetKey={targetSelections[item.card ?? 0] ?? ""} onTarget={(value) => setTargetSelections((current) => ({ ...current, [item.card ?? 0]: value }))} onClick={() => action(item)} />)}</div>
            {state.log.length > 0 && <details className="event-log"><summary>Recent engine receipts ({state.log.length})</summary><pre>{state.log.slice().reverse().join("\n")}</pre></details>}
          </div>
        </section>
      )}
    </main>
  );
}

function SeatPanel({ seat, opponent = false, label }: { seat: Seat; opponent?: boolean; label: string }) {
  return <section className={opponent ? "seat-panel opponent" : "seat-panel human"} aria-label={label}>
    <div className="seat-header"><div><div className="eyebrow">{opponent ? "OPPONENT" : "YOUR SEAT"}</div><h2>{label}</h2></div><div className="life-total">{seat.life}<small>life</small></div></div>
    <div className="zone-row"><div><div className="zone-title">LIBRARY</div><div className="hidden-zone">{seat.library_count}</div></div><div><div className="zone-title">GRAVEYARD</div><div className="hidden-zone">{seat.graveyard_count}</div></div><div><div className="zone-title">LANDS</div><div className="hidden-zone">{seat.lands_played}</div></div><ManaPool mana={seat.mana} /></div>
    <div className="zone-title battlefield-title">BATTLEFIELD</div><div className="cards battlefield">{seat.battlefield.length ? seat.battlefield.map((card) => <CardView card={card} key={card.id} />) : <div className="empty-zone">empty</div>}</div>
    {!opponent && <><div className="zone-title hand-title">HAND · {seat.hand.length}</div><div className="cards hand">{seat.hand.map((card) => <CardView card={card} key={card.id} hand />)}</div></>}
  </section>;
}

function ManaPool({ mana }: { mana: Record<string, number> }) {
  const symbols = [["white", "W"], ["blue", "U"], ["black", "B"], ["red", "R"], ["green", "G"], ["colorless", "C"]];
  return <div className="mana-pool"><div className="zone-title">MANA</div><div className="mana-symbols">{symbols.map(([key, symbol]) => <span className={`mana ${key}`} key={key}>{symbol}{mana[key] ?? 0}</span>)}</div></div>;
}

function CardView({ card, hand = false }: { card: Card; hand?: boolean }) {
  return <article className={`card ${hand ? "hand-card" : ""} ${card.tapped ? "tapped" : ""} ${card.colors.join(" ").toLowerCase()}`} aria-label={`${card.name}, ${card.mana_cost}`} data-card-id={card.id}>
    <div className="card-top"><strong>{card.name}</strong><span>{card.mana_cost}</span></div><div className="card-art">{card.power != null && card.toughness != null ? `${card.power}/${card.toughness}` : card.types[0] ?? "RAV"}</div><div className="card-bottom"><span>{card.types.join(" · ")}</span>{card.tapped && <b>TAPPED</b>}</div>
  </article>;
}

function ActionButton({ item, targetKey, onTarget, onClick }: { item: Action; targetKey: string; onTarget: (value: string) => void; onClick: () => void }) {
  return <div className="action-item"><button className={item.kind === "pass_priority" ? "secondary" : "primary"} onClick={onClick}>{item.label}</button>{item.kind === "cast" && item.targets.length > 0 && <select value={targetKey} onChange={(event) => onTarget(event.target.value)} aria-label={`Target for ${item.label}`}><option value="">Choose target…</option>{item.targets.map((target) => <option value={`${target.kind}:${target.id}`} key={`${target.kind}:${target.id}`}>{target.label}</option>)}</select>}</div>;
}

function CombatSelector({ cards, selected, setSelected, onSubmit, label }: { cards: Card[]; selected: number[]; setSelected: (ids: number[]) => void; onSubmit: () => void; label: string }) {
  return <div className="combat-selector"><div className="cards compact">{cards.map((card) => <button className={selected.includes(card.id) ? "card-select selected" : "card-select"} onClick={() => setSelected(selected.includes(card.id) ? selected.filter((id) => id !== card.id) : [...selected, card.id])} key={card.id}><CardView card={card} /></button>)}</div><button className="primary" onClick={onSubmit}>{label} ({selected.length})</button></div>;
}
