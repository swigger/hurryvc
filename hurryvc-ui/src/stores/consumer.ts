import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import type {
  InputKey,
  SessionSummary,
  TerminalSnapshot,
  WireMessage,
} from '@/lib/protocol'
import { applyDelta } from '@/lib/protocol'

const PROTOCOL_VERSION = 1 as const
const MASTER_KEY_STORAGE = 'hurryvc.master_key'
const GROUP_KEY_STORAGE = 'hurryvc.group_key'
const consumerSessionKey = createSessionKey()

export const useConsumerStore = defineStore('consumer', () => {
  const masterKey = ref(loadSaved(MASTER_KEY_STORAGE))
  const groupKey = ref(loadSaved(GROUP_KEY_STORAGE))
  const connectionState = ref<'idle' | 'connecting' | 'ready' | 'disconnected'>('idle')
  const lastError = ref<string | null>(null)
  const sessions = ref<SessionSummary[]>([])
  const selectedProducerId = ref<string | null>(null)
  const activeSession = ref<SessionSummary | null>(null)
  const activeSnapshot = ref<TerminalSnapshot | null>(null)
  const exitReason = ref<string | null>(null)
  const exitStatus = ref<number | null>(null)
  const currentSessionMissing = ref(false)
  const welcomed = ref(false)

  let socket: WebSocket | null = null
  let reconnectTimer: number | null = null

  const hasCredentials = computed(() => masterKey.value.trim().length > 0 && groupKey.value.trim().length > 0)
  const isTerminated = computed(() => exitReason.value !== null)
  const selectedSessionLabel = computed(() => activeSession.value?.producer_name ?? selectedProducerId.value ?? 'session')

  function saveKeys(nextMasterKey: string, nextGroupKey: string) {
    masterKey.value = nextMasterKey.trim()
    groupKey.value = nextGroupKey.trim()
    localStorage.setItem(MASTER_KEY_STORAGE, masterKey.value)
    localStorage.setItem(GROUP_KEY_STORAGE, groupKey.value)
  }

  function clearSavedKeys() {
    masterKey.value = ''
    groupKey.value = ''
    localStorage.removeItem(MASTER_KEY_STORAGE)
    localStorage.removeItem(GROUP_KEY_STORAGE)
  }

  function connect() {
    if (!hasCredentials.value) {
      return
    }
    if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
      return
    }
    clearReconnectTimer()
    connectionState.value = 'connecting'
    welcomed.value = false
    const nextSocket = new WebSocket(consumerWsUrl())
    socket = nextSocket
    nextSocket.addEventListener('open', () => {
      send({
        type: 'consumer_hello',
        version: PROTOCOL_VERSION,
        payload: {
          master_key: masterKey.value,
          production_group_key: groupKey.value,
          consumer_session_key: consumerSessionKey,
          client_info: navigator.userAgent,
        },
      })
    })
    nextSocket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data as string) as WireMessage
      handleMessage(message)
    })
    nextSocket.addEventListener('close', () => {
      socket = null
      welcomed.value = false
      connectionState.value = 'disconnected'
      scheduleReconnect()
    })
    nextSocket.addEventListener('error', () => {
      lastError.value = 'WebSocket connection failed'
    })
  }

  function disconnect() {
    clearReconnectTimer()
    socket?.close()
    socket = null
    welcomed.value = false
    connectionState.value = 'idle'
  }

  function ensureSelected(producerId: string) {
    const changed = selectedProducerId.value !== producerId
    selectedProducerId.value = producerId
    currentSessionMissing.value = false
    exitReason.value = null
    exitStatus.value = null
    if (changed) {
      activeSnapshot.value = null
    }
    activeSession.value = sessions.value.find((session) => session.producer_id === producerId) ?? activeSession.value
    connect()
    if (welcomed.value) {
      subscribeToSelected()
    }
  }

  function leaveSelected() {
    if (selectedProducerId.value && welcomed.value) {
      send({
        type: 'unsubscribe_session',
        version: PROTOCOL_VERSION,
        payload: {
          producer_id: selectedProducerId.value,
        },
      })
    }
    selectedProducerId.value = null
    activeSession.value = null
    activeSnapshot.value = null
    exitReason.value = null
    exitStatus.value = null
    currentSessionMissing.value = false
  }

  function refreshSessions() {
    connect()
    if (welcomed.value && selectedProducerId.value) {
      subscribeToSelected()
    }
  }

  function sendText(text: string) {
    if (!selectedProducerId.value) {
      return
    }
    send({
      type: 'consumer_input',
      version: PROTOCOL_VERSION,
      payload: {
        producer_id: selectedProducerId.value,
        input: {
          kind: 'text',
          data: text,
        },
      },
    })
  }

  function sendKey(key: InputKey) {
    if (!selectedProducerId.value) {
      return
    }
    send({
      type: 'consumer_input',
      version: PROTOCOL_VERSION,
      payload: {
        producer_id: selectedProducerId.value,
        input: {
          kind: 'key',
          key,
        },
      },
    })
  }

  function handleMessage(message: WireMessage) {
    switch (message.type) {
      case 'consumer_welcome':
        welcomed.value = true
        connectionState.value = 'ready'
        lastError.value = null
        if (selectedProducerId.value) {
          subscribeToSelected()
        }
        break
      case 'session_list':
        sessions.value = message.payload.sessions
        if (selectedProducerId.value) {
          const session = sessions.value.find((item) => item.producer_id === selectedProducerId.value) ?? null
          if (session) {
            activeSession.value = session
            currentSessionMissing.value = false
          } else if (!activeSnapshot.value) {
            currentSessionMissing.value = true
          }
        }
        break
      case 'term_snapshot':
        if (message.payload.producer_id !== selectedProducerId.value) {
          break
        }
        activeSnapshot.value = message.payload.snapshot
        exitReason.value = message.payload.snapshot.exit_status === null ? null : 'process exited'
        exitStatus.value = message.payload.snapshot.exit_status
        currentSessionMissing.value = false
        break
      case 'term_delta':
        if (message.payload.producer_id !== selectedProducerId.value) {
          break
        }
        activeSnapshot.value = activeSnapshot.value
          ? applyDelta(activeSnapshot.value, message.payload.delta)
          : {
              revision: message.payload.delta.revision,
              cols: message.payload.delta.cols,
              rows: message.payload.delta.rows,
              cursor_row: message.payload.delta.cursor_row,
              cursor_col: message.payload.delta.cursor_col,
              cursor_visible: message.payload.delta.cursor_visible,
              title: message.payload.delta.title,
              lines: message.payload.delta.lines,
              exit_status: message.payload.delta.exit_status,
            }
        exitStatus.value = message.payload.delta.exit_status
        break
      case 'session_terminated':
        if (message.payload.producer_id !== selectedProducerId.value) {
          break
        }
        if (message.payload.snapshot) {
          activeSnapshot.value = message.payload.snapshot
        }
        exitReason.value = message.payload.reason
        exitStatus.value = message.payload.exit_status
        currentSessionMissing.value = false
        break
      case 'consumer_error':
        lastError.value = message.payload.message
        if (message.payload.message.includes('not found') && !activeSnapshot.value) {
          currentSessionMissing.value = true
        }
        break
      case 'server_kick':
        lastError.value = message.payload.message
        disconnect()
        break
      default:
        break
    }
  }

  function subscribeToSelected() {
    if (!selectedProducerId.value) {
      return
    }
    send({
      type: 'subscribe_session',
      version: PROTOCOL_VERSION,
      payload: {
        producer_id: selectedProducerId.value,
      },
    })
  }

  function send(message: WireMessage) {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return
    }
    socket.send(JSON.stringify(message))
  }

  function scheduleReconnect() {
    if (!hasCredentials.value || reconnectTimer !== null) {
      return
    }
    reconnectTimer = window.setTimeout(() => {
      reconnectTimer = null
      connect()
    }, 1500)
  }

  function clearReconnectTimer() {
    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer)
      reconnectTimer = null
    }
  }

  return {
    activeSession,
    activeSnapshot,
    applyServerMessage: handleMessage,
    clearSavedKeys,
    connect,
    connectionState,
    currentSessionMissing,
    disconnect,
    ensureSelected,
    exitReason,
    exitStatus,
    groupKey,
    hasCredentials,
    isTerminated,
    lastError,
    leaveSelected,
    masterKey,
    refreshSessions,
    saveKeys,
    selectedProducerId,
    selectedSessionLabel,
    sendKey,
    sendText,
    sessions,
  }
})

function loadSaved(key: string) {
  return localStorage.getItem(key) ?? ''
}

function consumerWsUrl() {
  const url = new URL(window.location.href)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  const basePath = window.location.pathname.replace(/\/+$/, '')
  url.pathname = `${basePath || ''}/ws/consumer`
  url.hash = ''
  url.search = ''
  return url.toString()
}

function createSessionKey() {
  const bytes = crypto.getRandomValues(new Uint8Array(16))
  const encoded = btoa(String.fromCharCode(...bytes)).replace(/[+/=]/g, '')
  return `csk-${encoded}`
}
