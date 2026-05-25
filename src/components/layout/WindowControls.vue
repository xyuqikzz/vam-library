<template>
  <div class="window-controls">
    <button
      class="control-button close"
      aria-label="Close"
      @click="handleClose"
    >
      <svg width="8" height="8" viewBox="0 0 8 8" fill="none" class="control-icon">
        <path d="M1 1L7 7M7 1L1 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
    </button>
    <button
      class="control-button minimize"
      aria-label="Minimize"
      @click="handleMinimize"
    >
      <svg width="8" height="8" viewBox="0 0 8 8" fill="none" class="control-icon">
        <path d="M1 4H7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
    </button>
    <button
      class="control-button maximize"
      aria-label="Maximize"
      @click="handleMaximize"
    >
      <svg width="8" height="8" viewBox="0 0 8 8" fill="none" class="control-icon">
        <rect x="1" y="1" width="6" height="6" rx="0.5" stroke="currentColor" stroke-width="1.2" />
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

function handleClose() {
  appWindow.close()
}

function handleMinimize() {
  appWindow.minimize()
}

function handleMaximize() {
  appWindow.toggleMaximize()
}
</script>

<style scoped>
.window-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  height: var(--titlebar-height);
  position: absolute;
  top: 0;
  left: 0;
  z-index: var(--z-titlebar);
  -webkit-app-region: no-drag;
}

.control-button {
  position: relative;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    opacity var(--duration-fast) var(--ease),
    filter var(--duration-fast) var(--ease);
}

.control-icon {
  opacity: 0;
  transition: opacity var(--duration-fast) var(--ease);
  color: rgba(0, 0, 0, 0.65);
}

.window-controls:hover .control-icon {
  opacity: 1;
}

.control-button.close {
  background: #ff5f57;
}

.control-button.minimize {
  background: #febc2e;
}

.control-button.maximize {
  background: #28c840;
}

.control-button:hover {
  filter: brightness(1.1);
}

.control-button:active {
  filter: brightness(0.85);
}

/* When window is not focused, dim the controls */
.window-controls.blurred .control-button {
  background: var(--bg-hover);
}

.window-controls.blurred .control-icon {
  display: none;
}
</style>
