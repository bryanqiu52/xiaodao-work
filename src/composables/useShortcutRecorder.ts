// 快捷键录入：点「录入」后按组合键，自动拼成 `Ctrl+Shift+Space` 这种形态并保存。
//
// 两个细节：
//   - 只有修饰键按下时不提交（否则按下 Ctrl 就变成 "Ctrl" 存进去了）；
//   - 用 `event.code` 而不是 `event.key` 认键：`key` 会随输入法 / 键盘布局变，
//     同一个物理键在中文输入法下可能给出完全不同的字符。

import { ref } from 'vue'

const MODIFIERS = ['Control', 'Alt', 'Shift', 'Meta'] as const

/** 从物理键位还原出可读的键名 */
function keyFromEvent(e: KeyboardEvent): string {
  const code = e.code
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  const map: Record<string, string> = {
    Backquote: '`',
    Minus: '-',
    Equal: '=',
    BracketLeft: '[',
    BracketRight: ']',
    Backslash: '\\',
    Semicolon: ';',
    Quote: "'",
    Comma: ',',
    Period: '.',
    Slash: '/',
    Space: 'Space',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right',
    Escape: 'Esc',
  }
  if (map[code]) return map[code] ?? code
  // F1–F12 等直接用 key
  if (/^F\d{1,2}$/.test(e.key)) return e.key
  return e.key.length === 1 ? e.key.toUpperCase() : e.key
}

export function normalizeShortcutDisplay(v: string): string {
  return v.trim()
}

export function useShortcutRecorder(opts: {
  initial: string
  save: (v: string) => Promise<string | null>
  showToast: (msg: string) => void
}) {
  const value = ref(opts.initial)
  const saved = ref(opts.initial)
  const listening = ref(false)
  const error = ref('')

  function startListening(): void {
    error.value = ''
    listening.value = true
    value.value = ''
  }

  async function commit(): Promise<void> {
    listening.value = false
    const v = value.value.trim()
    if (v.length === 0) {
      value.value = saved.value
      return
    }
    const err = await opts.save(v)
    if (err === null) {
      saved.value = v
      opts.showToast(`快捷键已设为 ${v}`)
    } else {
      error.value = err
      value.value = saved.value
    }
  }

  function onKeydown(e: KeyboardEvent): void {
    if (!listening.value) return
    e.preventDefault()
    e.stopPropagation()
    // 只按修饰键时不算数，等真正的主键
    if (MODIFIERS.includes(e.key as (typeof MODIFIERS)[number])) return
    const parts: string[] = []
    if (e.ctrlKey) parts.push('Ctrl')
    if (e.altKey) parts.push('Alt')
    if (e.shiftKey) parts.push('Shift')
    if (e.metaKey) parts.push('Super')
    parts.push(keyFromEvent(e))
    value.value = parts.join('+')
    void commit()
  }

  function onBlur(): void {
    if (listening.value) {
      listening.value = false
      value.value = saved.value
    }
  }

  return { value, listening, error, startListening, commit, onKeydown, onBlur }
}
