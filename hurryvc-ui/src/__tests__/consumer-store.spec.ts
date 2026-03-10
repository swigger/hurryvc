import { beforeEach, describe, expect, it } from 'vitest'

import { createPinia, setActivePinia } from 'pinia'

import type { SessionSummary, TerminalSnapshot } from '@/lib/protocol'
import { useConsumerStore } from '@/stores/consumer'

const session: SessionSummary = {
  producer_id: 'prd-1',
  producer_name: 'codex',
  command: ['codex'],
  platform: 'windows',
  cols: 120,
  rows: 40,
  cwd: 'D:/work',
  pid: 99,
  streaming: true,
}

const snapshot: TerminalSnapshot = {
  revision: 1,
  cols: 120,
  rows: 40,
  cursor_row: 0,
  cursor_col: 0,
  cursor_visible: true,
  title: null,
  lines: [{ index: 0, runs: [{ text: 'hello', fg: null, bg: null, bold: false, dim: false, italic: false, underline: false, inverse: false }], wrapped: false }],
  exit_status: null,
}

describe('consumer store', () => {
  beforeEach(() => {
    localStorage.clear()
    setActivePinia(createPinia())
  })

  it('keeps the last terminal snapshot after exit until page refresh', () => {
    const store = useConsumerStore()
    store.selectedProducerId = 'prd-1'
    store.applyServerMessage({
      type: 'session_list',
      version: 1,
      payload: { sessions: [session] },
    })
    store.applyServerMessage({
      type: 'term_snapshot',
      version: 1,
      payload: {
        producer_id: 'prd-1',
        snapshot,
      },
    })
    store.applyServerMessage({
      type: 'session_terminated',
      version: 1,
      payload: {
        producer_id: 'prd-1',
        snapshot: {
          ...snapshot,
          exit_status: 0,
        },
        exit_status: 0,
        reason: 'process exited',
      },
    })
    store.applyServerMessage({
      type: 'session_list',
      version: 1,
      payload: { sessions: [] },
    })

    expect(store.activeSnapshot?.exit_status).toBe(0)
    expect(store.exitReason).toBe('process exited')
    expect(store.currentSessionMissing).toBe(false)
  })

  it('does not recover an exited session after a fresh store boot', () => {
    const store = useConsumerStore()
    store.selectedProducerId = 'prd-1'
    store.applyServerMessage({
      type: 'session_list',
      version: 1,
      payload: { sessions: [] },
    })

    expect(store.activeSnapshot).toBeNull()
    expect(store.currentSessionMissing).toBe(true)
  })
})
