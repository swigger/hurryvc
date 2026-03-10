<script setup lang="ts">
import { computed, onMounted, reactive, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { InputKey } from '@/lib/protocol'
import {
  TERMINAL_DEFAULT_FG,
  runStyle,
} from '@/lib/terminal-theme'
import { useConsumerStore } from '@/stores/consumer'

const route = useRoute()
const router = useRouter()
const store = useConsumerStore()
const form = reactive({
  text: '',
})

const producerId = computed(() => String(route.params.producerId ?? ''))
const controlKeys: Array<{ label: string; key: InputKey }> = [
  { label: 'Enter', key: 'enter' },
  { label: 'Tab', key: 'tab' },
  { label: 'Backspace', key: 'backspace' },
  { label: 'Esc', key: 'escape' },
  { label: 'Up', key: 'arrow_up' },
  { label: 'Down', key: 'arrow_down' },
  { label: 'Left', key: 'arrow_left' },
  { label: 'Right', key: 'arrow_right' },
  { label: 'Ctrl+C', key: 'ctrl_c' },
  { label: 'Ctrl+D', key: 'ctrl_d' },
]

onMounted(() => {
  if (!store.hasCredentials) {
    void router.replace('/')
    return
  }
  store.ensureSelected(producerId.value)
})

watch(producerId, (nextProducerId) => {
  if (nextProducerId) {
    store.ensureSelected(nextProducerId)
  }
})

watch(
  () => store.currentSessionMissing && !store.activeSnapshot,
  (missing) => {
    if (missing) {
      void router.replace('/sessions')
    }
  },
)

function submitText() {
  if (!form.text) {
    return
  }
  store.sendText(form.text)
  form.text = ''
}

function goBack() {
  store.leaveSelected()
  void router.push('/sessions')
}

</script>

<template>
  <section class="terminal-page">
    <header class="terminal-header">
      <div>
        <p class="eyebrow">Terminal</p>
        <h1>{{ store.selectedSessionLabel }}</h1>
        <p class="sub">
          连接状态: {{ store.connectionState }}
          <span v-if="store.exitReason"> | 状态: {{ store.exitReason }}</span>
          <span v-if="store.exitStatus !== null"> | exit={{ store.exitStatus }}</span>
        </p>
      </div>
      <div class="header-actions">
        <button class="ghost" @click="goBack">返回列表</button>
      </div>
    </header>

    <div v-if="store.activeSnapshot" class="terminal-shell">
      <div class="terminal-meta">
        <span>{{ store.activeSnapshot.cols }} x {{ store.activeSnapshot.rows }}</span>
        <span>revision {{ store.activeSnapshot.revision }}</span>
      </div>
      <div class="terminal-surface">
        <div
          v-for="line in store.activeSnapshot.lines"
          :key="line.index"
          class="terminal-line"
        >
          <template v-for="(run, index) in line.runs" :key="`${line.index}-${index}`">
            <span :style="runStyle(run)">{{ run.text }}</span>
          </template>
        </div>
      </div>
    </div>

    <div v-else class="terminal-empty">
      <p>等待终端快照...</p>
    </div>

    <form class="input-panel" @submit.prevent="submitText">
      <label>
        <span>批量输入</span>
        <textarea
          v-model="form.text"
          rows="5"
          placeholder="在这里整理好要发送的文本，再一次性提交"
        />
      </label>
      <div class="input-actions">
        <button type="submit">发送文本</button>
      </div>
    </form>

    <div class="control-grid">
      <button
        v-for="control in controlKeys"
        :key="control.key"
        class="ghost"
        @click="store.sendKey(control.key)"
      >
        {{ control.label }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.terminal-page {
  display: grid;
  gap: 18px;
}

.terminal-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-end;
  flex-wrap: wrap;
}

.eyebrow {
  margin: 0 0 8px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #597266;
  font-size: 0.8rem;
}

h1, p {
  margin: 0;
}

.sub {
  margin-top: 8px;
  color: #587165;
}

.terminal-shell {
  display: grid;
  gap: 10px;
}

.terminal-meta {
  display: flex;
  gap: 12px;
  color: #587165;
  font-size: 0.88rem;
}

.terminal-surface {
  --terminal-fg: v-bind(TERMINAL_DEFAULT_FG);
  min-height: 360px;
  padding: 18px;
  border-radius: 24px;
  background:
    radial-gradient(circle at top right, rgba(29, 75, 59, 0.35), transparent 26%),
    linear-gradient(180deg, #101915, #0a110f);
  color: var(--terminal-fg);
  font-family: 'Cascadia Mono', 'Consolas', monospace;
  white-space: pre;
  overflow: auto;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
}

.terminal-line {
  min-height: 1.35em;
}

.terminal-empty {
  padding: 32px;
  border-radius: 24px;
  background: rgba(255, 255, 255, 0.72);
  color: #587165;
}

.input-panel {
  display: grid;
  gap: 12px;
  padding: 20px;
  border-radius: 24px;
  background: rgba(255, 255, 255, 0.72);
  border: 1px solid rgba(12, 32, 28, 0.1);
}

label {
  display: grid;
  gap: 8px;
}

textarea {
  width: 100%;
  border: 1px solid rgba(12, 32, 28, 0.16);
  border-radius: 16px;
  padding: 14px;
  resize: vertical;
  font: inherit;
}

.input-actions,
.header-actions,
.control-grid {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

button {
  border: 0;
  border-radius: 999px;
  padding: 12px 18px;
  background: #0e3b2f;
  color: white;
  font-weight: 600;
  cursor: pointer;
}

.ghost {
  background: rgba(14, 59, 47, 0.08);
  color: #0e3b2f;
}
</style>
