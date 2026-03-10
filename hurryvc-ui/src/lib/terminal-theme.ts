import type { TerminalColor, TerminalRun } from '@/lib/protocol'

export const TERMINAL_DEFAULT_FG = '#dcdcdc'
export const TERMINAL_MUTED_FG = '#767676'

export function runStyle(run: TerminalRun) {
  const style: Record<string, string | number> = {}
  const fg = colorValue(run.inverse ? run.bg : run.fg)
  const bg = colorValue(run.inverse ? run.fg : run.bg)

  if (run.dim && !fg) {
    style.color = TERMINAL_MUTED_FG
  } else if (fg) {
    style.color = fg
  }

  if (bg) {
    style.backgroundColor = bg
  }
  if (run.bold) {
    style.fontWeight = 700
  }
  if (run.dim && fg) {
    style.opacity = 0.52
  }
  if (run.italic) {
    style.fontStyle = 'italic'
  }
  if (run.underline) {
    style.textDecoration = 'underline'
  }
  return style
}

export function colorValue(color: TerminalColor | null) {
  if (!color) {
    return null
  }
  if (color.kind === 'rgb') {
    return `rgb(${color.r}, ${color.g}, ${color.b})`
  }
  return indexedColor(color.value)
}

export function indexedColor(value: number) {
  const palette = [
    '#1b1c1d', '#ba3a48', '#1f7a57', '#af7b00',
    '#1b6ca8', '#865ab9', '#00877b', '#c6d6cf',
    '#5a625e', '#f16f7a', '#44bf8a', '#f0b34a',
    '#5aa7f0', '#b88cf2', '#49c7bb', '#f4f7f5',
  ]
  if (value < palette.length) {
    return palette[value]
  }
  if (value >= 232) {
    const gray = 8 + (value - 232) * 10
    return `rgb(${gray}, ${gray}, ${gray})`
  }
  const offset = value - 16
  const r = Math.floor(offset / 36)
  const g = Math.floor((offset % 36) / 6)
  const b = offset % 6
  const scale = [0, 95, 135, 175, 215, 255]
  return `rgb(${scale[r]}, ${scale[g]}, ${scale[b]})`
}
