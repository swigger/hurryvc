import { describe, expect, it } from 'vitest'

import type { TerminalRun } from '@/lib/protocol'
import {
  TERMINAL_MUTED_FG,
  indexedColor,
  runStyle,
} from '@/lib/terminal-theme'

function baseRun(): TerminalRun {
  return {
    text: 'demo',
    fg: null,
    bg: null,
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    inverse: false,
  }
}

describe('terminal theme', () => {
  it('renders dim default text as muted gray instead of inheriting bright default fg', () => {
    const style = runStyle({
      ...baseRun(),
      dim: true,
    })

    expect(style.color).toBe(TERMINAL_MUTED_FG)
    expect(style.opacity).toBeUndefined()
  })

  it('keeps indexed bright black mapped to a visible gray', () => {
    expect(indexedColor(8)).toBe('#5a625e')
  })

  it('uses dim opacity for explicitly colored faint text', () => {
    const style = runStyle({
      ...baseRun(),
      dim: true,
      italic: true,
      fg: { kind: 'indexed', value: 15 },
    })

    expect(style.color).toBe('#f4f7f5')
    expect(style.opacity).toBe(0.76)
  })
})
