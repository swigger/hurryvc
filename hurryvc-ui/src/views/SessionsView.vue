<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'

import { useConsumerStore } from '@/stores/consumer'

const router = useRouter()
const store = useConsumerStore()

onMounted(() => {
  if (!store.hasCredentials) {
    void router.replace('/')
    return
  }
  store.connect()
})

function openSession(producerId: string) {
  store.ensureSelected(producerId)
  void router.push(`/sessions/${producerId}`)
}
</script>

<template>
  <section class="sessions-page">
    <header class="page-header">
      <div>
        <p class="eyebrow">Session list</p>
        <h1>可接管的终端</h1>
      </div>
      <div class="header-actions">
        <button class="ghost" @click="store.refreshSessions()">刷新</button>
        <button class="ghost" @click="store.clearSavedKeys(); router.push('/')">退出并清空密钥</button>
      </div>
    </header>

    <p class="status">连接状态: {{ store.connectionState }}</p>
    <p v-if="store.lastError" class="error">{{ store.lastError }}</p>

    <div v-if="store.sessions.length === 0" class="empty">
      当前 group 下没有活动 session。
    </div>

    <ul v-else class="session-grid">
      <li v-for="session in store.sessions" :key="session.producer_id" class="session-card">
        <div class="session-head">
          <div>
            <h2>{{ session.producer_name }}</h2>
            <p>{{ session.command.join(' ') }}</p>
          </div>
          <span class="pill" :class="{ live: session.streaming }">
            {{ session.streaming ? 'streaming' : 'idle' }}
          </span>
        </div>
        <dl>
          <div>
            <dt>Platform</dt>
            <dd>{{ session.platform }}</dd>
          </div>
          <div>
            <dt>Size</dt>
            <dd>{{ session.cols }} x {{ session.rows }}</dd>
          </div>
          <div>
            <dt>PID</dt>
            <dd>{{ session.pid }}</dd>
          </div>
          <div>
            <dt>CWD</dt>
            <dd>{{ session.cwd ?? '-' }}</dd>
          </div>
        </dl>
        <button @click="openSession(session.producer_id)">打开终端</button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.sessions-page {
  display: grid;
  gap: 20px;
}

.page-header {
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

h1, h2, p {
  margin: 0;
}

.header-actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.status, .error {
  margin: 0;
}

.error {
  color: #a12630;
}

.empty {
  padding: 40px;
  border-radius: 24px;
  background: rgba(255, 255, 255, 0.68);
  border: 1px dashed rgba(12, 32, 28, 0.18);
  color: #597266;
}

.session-grid {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 18px;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
}

.session-card {
  display: grid;
  gap: 18px;
  padding: 24px;
  border-radius: 24px;
  background: rgba(255, 255, 255, 0.76);
  border: 1px solid rgba(12, 32, 28, 0.14);
  box-shadow: 0 18px 38px rgba(12, 32, 28, 0.08);
}

.session-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.pill {
  padding: 6px 10px;
  border-radius: 999px;
  background: rgba(76, 95, 87, 0.12);
  color: #4c5f57;
  font-size: 0.82rem;
}

.pill.live {
  background: rgba(31, 105, 73, 0.14);
  color: #1f6949;
}

dl {
  display: grid;
  gap: 12px;
  margin: 0;
}

dt {
  color: #597266;
  font-size: 0.82rem;
}

dd {
  margin: 4px 0 0;
  color: #12231d;
  word-break: break-word;
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
