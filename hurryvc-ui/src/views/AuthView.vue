<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useConsumerStore } from '@/stores/consumer'

const store = useConsumerStore()
const router = useRouter()
const form = reactive({
  masterKey: store.masterKey,
  groupKey: store.groupKey,
})

const showMasterKey = ref(false)
const showGroupKey = ref(false)

function submit() {
  store.saveKeys(form.masterKey, form.groupKey)
  store.connect()
  void router.push('/sessions')
}
</script>

<template>
  <section class="auth-shell">
    <div class="auth-card">
      <p class="eyebrow">hurryvc consumer</p>
      <h1>接管正在运行的终端</h1>
      <p class="lead">消费端只需要浏览器。登录后会建立一条长连接，进入你当前分组下的活动 session。</p>

      <form class="auth-form" @submit.prevent="submit">
        <label>
          <span>Master key</span>
          <div class="input-wrapper">
            <input v-model="form.masterKey" :type="showMasterKey ? 'text' : 'password'" autocomplete="off" />
            <button type="button" class="eye-btn" @click="showMasterKey = !showMasterKey">
              <svg v-if="showMasterKey" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                <circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                <line x1="1" y1="1" x2="23" y2="23"/>
              </svg>
            </button>
          </div>
        </label>
        <label>
          <span>Production group key</span>
          <div class="input-wrapper">
            <input v-model="form.groupKey" :type="showGroupKey ? 'text' : 'password'" autocomplete="off" />
            <button type="button" class="eye-btn" @click="showGroupKey = !showGroupKey">
              <svg v-if="showGroupKey" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                <circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                <line x1="1" y1="1" x2="23" y2="23"/>
              </svg>
            </button>
          </div>
        </label>

        <div class="actions">
          <button type="submit">进入控制台</button>
          <button type="button" class="ghost" @click="store.clearSavedKeys()">清空本地密钥</button>
        </div>
      </form>

      <p v-if="store.lastError" class="error">{{ store.lastError }}</p>
    </div>
  </section>
</template>

<style scoped>
.auth-shell {
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 24px;
}

.auth-card {
  width: min(100%, 560px);
  padding: 32px;
  border: 1px solid rgba(12, 32, 28, 0.15);
  border-radius: 28px;
  background:
    radial-gradient(circle at top left, rgba(229, 243, 214, 0.9), transparent 35%),
    linear-gradient(160deg, rgba(255, 250, 241, 0.94), rgba(243, 247, 239, 0.96));
  box-shadow: 0 22px 60px rgba(12, 32, 28, 0.12);
}

.eyebrow {
  margin: 0 0 12px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  font-size: 0.8rem;
  color: #486155;
}

h1 {
  margin: 0;
  font-size: clamp(2rem, 4vw, 3rem);
  line-height: 1.05;
}

.lead {
  margin: 12px 0 0;
  color: #365045;
}

.auth-form {
  display: grid;
  gap: 16px;
  margin-top: 28px;
}

label {
  display: grid;
  gap: 8px;
  color: #1d2e28;
}

.input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

input {
  width: 100%;
  border: 1px solid rgba(12, 32, 28, 0.2);
  border-radius: 14px;
  padding: 14px 48px 14px 16px;
  background: rgba(255, 255, 255, 0.76);
}

.eye-btn {
  position: absolute;
  right: 12px;
  padding: 4px;
  background: transparent;
  color: #486155;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.eye-btn:hover {
  background: rgba(12, 32, 28, 0.08);
}

.actions {
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

.error {
  margin-top: 16px;
  color: #9d2933;
}
</style>
