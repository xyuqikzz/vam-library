<template>
  <div class="health-status">
    <!-- All clear -->
    <div v-if="totalIssues === 0" class="health-ok">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
        <path d="M20 6L9 17l-5-5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span>{{ $t('dashboard.noHealthData') }}</span>
    </div>

    <!-- Has issues -->
    <div v-else class="health-issues">
      <!-- Missing Dependencies -->
      <div v-if="missing > 0" class="health-group">
        <button class="health-row" @click="showMissing = !showMissing">
          <div class="health-row-left">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
              <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5"/>
              <path d="M12 8v4M12 16h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
            <span>{{ $t('dashboard.missingDependency', { count: missing }) }}</span>
          </div>
          <svg
            width="12" height="12" viewBox="0 0 24 24" fill="none"
            class="chevron" :class="{ open: showMissing }"
          >
            <path d="M9 5l7 7-7 7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <Transition name="expand">
          <div v-if="showMissing && missingList.length > 0" class="health-detail">
            <div v-for="(dep, idx) in missingList" :key="idx" class="dep-row">
              <div class="dep-info">
                <span class="dep-name">{{ dep.depends_on_id }}</span>
                <span class="dep-ref">{{ $t('dashboard.referencedBy', { name: dep.package_id }) }}</span>
              </div>
              <button class="copy-btn" :title="$t('dashboard.copyId')" @click.stop="copyId(dep.depends_on_id)">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none">
                  <rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" stroke="currentColor" stroke-width="1.5"/>
                </svg>
              </button>
            </div>
          </div>
        </Transition>
      </div>

      <!-- Corrupted Packages -->
      <div v-if="corrupted > 0" class="health-group">
        <button class="health-row" @click="showCorrupted = !showCorrupted">
          <div class="health-row-left">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
              <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5"/>
              <path d="M12 8v4M12 16h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
            <span>{{ $t('dashboard.corruptedPackageCount', { count: corrupted }) }}</span>
          </div>
          <svg
            width="12" height="12" viewBox="0 0 24 24" fill="none"
            class="chevron" :class="{ open: showCorrupted }"
          >
            <path d="M9 5l7 7-7 7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <Transition name="expand">
          <div v-if="showCorrupted && corruptedList.length > 0" class="health-detail">
            <div v-for="(pkg, idx) in corruptedList" :key="idx" class="dep-row">
              <div class="dep-info">
                <span class="dep-name">{{ pkg.package_id }}</span>
                <span class="dep-ref">{{ pkg.error }}</span>
              </div>
            </div>
          </div>
        </Transition>
      </div>

      <!-- Duplicates -->
      <div v-if="duplicates > 0" class="health-row static">
        <div class="health-row-left">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
            <rect x="3" y="3" width="12" height="12" rx="2" stroke="currentColor" stroke-width="1.5"/>
            <path d="M9 9H21V21H9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span>{{ $t('dashboard.duplicateResource', { count: duplicates }) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useNotification } from '@/composables/useNotification'

const props = defineProps<{
  missing: number
  corrupted: number
  duplicates: number
  missingList: Array<{ depends_on_id: string; package_id: string }>
  corruptedList: Array<{ package_id: string; error: string }>
}>()

const { t } = useI18n()
const notify = useNotification()

const showMissing = ref(false)
const showCorrupted = ref(false)

const totalIssues = computed(() => props.missing + props.corrupted + props.duplicates)

async function copyId(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    notify.success(t('dashboard.idCopied'))
  } catch (e) {
    notify.error(String(e), t('common.error'))
  }
}
</script>

<style scoped>
.health-ok {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: var(--radius-md);
  background: rgba(48, 209, 88, 0.06);
  color: var(--color-success);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.health-issues {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.health-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.health-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  background: var(--color-warning-bg);
  color: var(--color-warning);
  font-size: var(--text-sm);
  cursor: pointer;
  border: none;
  width: 100%;
  text-align: left;
  transition: background var(--duration-fast) var(--ease);
}

.health-row:hover {
  background: rgba(255, 214, 10, 0.14);
}

.health-row.static {
  cursor: default;
}

.health-row-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.chevron {
  transition: transform var(--duration-base) var(--ease);
  opacity: 0.7;
}

.chevron.open {
  transform: rotate(90deg);
}

.health-detail {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-2);
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
  max-height: 240px;
  overflow-y: auto;
}

.dep-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: 4px var(--space-2);
  border-radius: var(--radius-xs);
  transition: background var(--duration-fast) var(--ease);
}

.dep-row:hover {
  background: rgba(255, 255, 255, 0.03);
}

.dep-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
  flex: 1;
}

.dep-name {
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  color: var(--text-primary);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dep-ref {
  font-size: 10px;
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.copy-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  background: transparent;
  border: none;
  cursor: pointer;
  flex-shrink: 0;
  transition: all var(--duration-fast) var(--ease);
}

.copy-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.copy-btn:active {
  transform: scale(0.9);
}

/* Expand transition */
.expand-enter-active,
.expand-leave-active {
  transition: all 200ms var(--ease);
  max-height: 240px;
  opacity: 1;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  max-height: 0;
  opacity: 0;
  padding: 0 !important;
}
</style>
