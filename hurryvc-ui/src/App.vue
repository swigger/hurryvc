<script setup lang="ts">
import { RouterView } from 'vue-router'
</script>

<template>
  <div class="app-shell">
    <div class="ambient ambient-a"></div>
    <div class="ambient ambient-b"></div>
    <main class="app-frame">
      <RouterView />
    </main>
  </div>
</template>

<style scoped>
:global(body) {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
  background:
    radial-gradient(circle at top left, rgba(212, 236, 188, 0.62), transparent 22%),
    radial-gradient(circle at bottom right, rgba(255, 215, 160, 0.5), transparent 18%),
    linear-gradient(160deg, #f7efe2, #edf4ed 55%, #e5efe8);
  color: #12231d;
  font-family: 'Aptos', 'Segoe UI', sans-serif;
}

:global(*) {
  box-sizing: border-box;
}

.app-shell {
  position: relative;
  min-height: 100vh;
  overflow: hidden;
}

.ambient {
  position: fixed;
  width: 42vw;
  height: 42vw;
  border-radius: 50%;
  filter: blur(48px);
  opacity: 0.28;
  pointer-events: none;
}

.ambient-a {
  top: -10vw;
  left: -12vw;
  background: #b7d68d;
}

.ambient-b {
  right: -12vw;
  bottom: -10vw;
  background: #f2c589;
}

.app-frame {
  position: relative;
  z-index: 1;
  width: min(1200px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 24px 0 40px;
}
</style>
