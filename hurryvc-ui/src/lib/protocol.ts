export type SessionSummary = {
  producer_id: string
  producer_name: string
  command: string[]
  platform: string
  cols: number
  rows: number
  cwd: string | null
  pid: number
  streaming: boolean
}

export type TerminalColor =
  | { kind: 'indexed'; value: number }
  | { kind: 'rgb'; r: number; g: number; b: number }

export type TerminalRun = {
  text: string
  fg: TerminalColor | null
  bg: TerminalColor | null
  bold: boolean
  dim: boolean
  italic: boolean
  underline: boolean
  inverse: boolean
}

export type TerminalLine = {
  index: number
  runs: TerminalRun[]
  wrapped: boolean
}

export type TerminalSnapshot = {
  revision: number
  cols: number
  rows: number
  cursor_row: number
  cursor_col: number
  cursor_visible: boolean
  title: string | null
  lines: TerminalLine[]
  exit_status: number | null
}

export type TerminalDelta = {
  revision: number
  cols: number
  rows: number
  cursor_row: number
  cursor_col: number
  cursor_visible: boolean
  title: string | null
  lines: TerminalLine[]
  exit_status: number | null
}

export type WireMessage =
  | { type: 'consumer_hello'; version: 1; payload: ConsumerHelloPayload }
  | { type: 'consumer_welcome'; version: 1; payload: { consumer_id: string } }
  | { type: 'consumer_ping'; version: 1 }
  | { type: 'subscribe_session'; version: 1; payload: { producer_id: string } }
  | { type: 'unsubscribe_session'; version: 1; payload: { producer_id: string } }
  | { type: 'consumer_input'; version: 1; payload: ConsumerInputPayload }
  | { type: 'term_snapshot'; version: 1; payload: { producer_id: string; snapshot: TerminalSnapshot } }
  | { type: 'term_delta'; version: 1; payload: { producer_id: string; delta: TerminalDelta } }
  | { type: 'session_list'; version: 1; payload: { sessions: SessionSummary[] } }
  | {
      type: 'session_terminated'
      version: 1
      payload: {
        producer_id: string
        snapshot: TerminalSnapshot | null
        exit_status: number | null
        reason: string
      }
    }
  | { type: 'consumer_error'; version: 1; payload: { message: string } }
  | { type: 'server_kick'; version: 1; payload: { message: string } }

export type ConsumerHelloPayload = {
  master_key: string
  production_group_key: string
  consumer_session_key: string
  client_info: string | null
}

export type ConsumerInputPayload = {
  producer_id: string
  input: TerminalInput
}

export type TerminalInput =
  | { kind: 'text'; data: string }
  | { kind: 'key'; key: InputKey }

export type InputKey =
  | 'enter'
  | 'tab'
  | 'backspace'
  | 'escape'
  | 'arrow_up'
  | 'arrow_down'
  | 'arrow_left'
  | 'arrow_right'
  | 'ctrl_c'
  | 'ctrl_d'

export function applyDelta(snapshot: TerminalSnapshot, delta: TerminalDelta): TerminalSnapshot {
  const nextLines = [...snapshot.lines]
  const targetLength = delta.rows
  while (nextLines.length < targetLength) {
    nextLines.push({ index: nextLines.length, runs: [], wrapped: false })
  }
  nextLines.length = targetLength
  for (const line of delta.lines) {
    nextLines[line.index] = line
  }
  return {
    revision: delta.revision,
    cols: delta.cols,
    rows: delta.rows,
    cursor_row: delta.cursor_row,
    cursor_col: delta.cursor_col,
    cursor_visible: delta.cursor_visible,
    title: delta.title,
    lines: nextLines,
    exit_status: delta.exit_status,
  }
}
