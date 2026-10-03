<template>
  <div class="migration-view animate-fadeIn">
    <!-- Top Action Bar -->
    <div class="migration-header-bar glass-panel">
      <!-- Step Indicator -->
      <div class="step-indicator">
        <div
          v-for="(step, index) in steps"
          :key="index"
          :class="['step', { active: currentStep === index, completed: currentStep > index }]"
        >
          <div class="step-number">
            <svg v-if="currentStep > index" width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M3 7L6 10L11 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span v-else>{{ index + 1 }}</span>
          </div>
          <span class="step-label">{{ step }}</span>
          <div v-if="index < steps.length - 1" class="step-connector" />
        </div>
      </div>

      <!-- Navigation Actions (Moved to Top) -->
      <div class="step-actions">
        <!-- Step 0 Actions -->
        <template v-if="currentStep === 0">
          <button
            class="step-rollback-btn"
            :disabled="isRestoringAll"
            @click="handleRestoreAll"
          >
            {{ isRestoringAll ? $t('migration.restoringAll') : $t('migration.restoreAll') }}
          </button>
          <button
            class="step-next-btn"
            :disabled="!selectedMode"
            @click="currentStep = 1"
          >
            {{ $t('common.next') }}
          </button>
        </template>

        <!-- Step 1 Actions -->
        <template v-else-if="currentStep === 1">
          <button class="step-back-btn" @click="currentStep = 0">
            {{ $t('common.back') }}
          </button>
          <button
            class="step-next-btn"
            @click="handlePreview"
          >
            {{ $t('common.next') }}
          </button>
        </template>

        <!-- Step 2 Actions -->
        <template v-else-if="currentStep === 2">
          <button class="step-back-btn" @click="currentStep = 1">
            {{ $t('common.back') }}
          </button>
          <button
            v-if="preview?.plan_id && (preview.operations.length > 0 || preview.cleanup_empty_dirs)"
            class="step-execute-btn"
            @click="handleExecute"
          >
            {{ $t('common.confirm') }}
          </button>
        </template>

        <!-- Step 3 Actions -->
        <template v-else-if="currentStep === 3 && migrationResult">
          <button class="step-back-btn" @click="resetWizard">
            {{ $t('migration.startNew') }}
          </button>
          <button
            v-if="migrationResult.rollback_available"
            class="step-rollback-btn"
            :disabled="isRollingBack"
            @click="handleRollback"
          >
            {{ isRollingBack ? $t('migration.rollingBack') : $t('migration.rollback') }}
          </button>
        </template>
      </div>
    </div>

    <!-- Step 0: Mode Selection -->
    <div v-if="currentStep === 0" class="step-content">
      <h2 class="step-title">{{ $t('migration.chooseMode') }}</h2>
      <p class="step-description text-secondary">{{ $t('migration.chooseModeDesc') }}</p>

      <div class="mode-grid stagger-children">
        <button
          v-for="mode in modes"
          :key="mode.id"
          :class="['mode-card glass-card-component', { selected: selectedMode === mode.id }]"
          @click="selectedMode = mode.id"
        >
          <div class="mode-icon-wrapper" :style="{ '--mode-color': mode.color }">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
              <path :d="mode.icon" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>
          <h3 class="mode-title">{{ mode.title }}</h3>
          <p class="mode-description">{{ mode.description }}</p>
          <div v-if="selectedMode === mode.id" class="selected-indicator">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5" />
              <path d="M5 8L7 10L11 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>
        </button>
      </div>


    </div>

    <!-- Step 1: Configure -->
    <div v-else-if="currentStep === 1" class="step-content">
      <h2 class="step-title">{{ $t('migration.configureTitle') }}</h2>
      <p class="step-description text-secondary">{{ $t('migration.configureDesc') }}</p>

      <div class="config-form">
        <div class="config-field">
          <label class="config-label text-sm">{{ $t('migration.sourceDirectory') }}</label>
          <div class="config-input-row">
            <input
              v-model="config.sourceDir"
              type="text"
              class="config-input"
              :placeholder="$t('migration.sourceDirectoryPlaceholder')"
            />
            <span class="config-hint text-xs text-tertiary">{{ $t('migration.sourceDirectoryHint') }}</span>
          </div>
        </div>

        <div class="config-field">
          <label class="config-label text-sm">{{ $t('migration.targetDirectory') }}</label>
          <div class="target-option-row">
            <button
              v-for="option in targetOptions"
              :key="option.id"
              :class="['config-toggle', { active: config.targetKind === option.id }]"
              @click="config.targetKind = option.id"
            >
              {{ option.label }}
            </button>
          </div>
          <div class="selected-target">
            <span class="selected-target-path truncate">{{ selectedTargetDir || $t('migration.targetDirectoryPlaceholder') }}</span>
          </div>
          <span class="config-hint text-xs text-tertiary">{{ $t('migration.targetDirectoryHint') }}</span>
        </div>

        <div class="config-field">
          <label class="config-label text-sm">{{ $t('migration.operation') }}</label>
          <div class="config-toggle-row">
            <button
              :class="['config-toggle', { active: config.action === 'move' }]"
              @click="config.action = 'move'"
            >
              {{ $t('migration.move') }}
            </button>
            <button
              :class="['config-toggle', { active: config.action === 'copy' }]"
              @click="config.action = 'copy'"
            >
              {{ $t('migration.copySafe') }}
            </button>
          </div>
        </div>
      </div>


    </div>

    <!-- Step 2: Preview -->
    <div v-else-if="currentStep === 2" class="step-content">
      <h2 class="step-title">{{ $t('migration.previewTitle') }}</h2>
      <p class="step-description text-secondary">
        {{ $t('migration.previewDesc', { count: preview?.total_operations ?? 0, action: config.action === 'copy' ? $t('migration.actionCopy') : $t('migration.actionMove'), size: formatSize(preview?.total_size_bytes ?? 0) }) }}
      </p>

      <p v-if="preview" class="text-sm text-secondary">{{ t('migration.diskSummary', { count: preview.total_files, skipped: preview.skipped }) }}</p>
      <p v-if="preview?.cleanup_empty_dirs" class="text-sm text-secondary">{{ t('migration.emptyDirHint') }}</p>
      <details v-if="preview?.warnings.length" class="preview-warnings">
        <summary>{{ t('migration.scanWarnings', { count: preview.warnings.length }) }}</summary>
        <p v-for="warning in preview.warnings" :key="warning" class="text-xs">{{ warning }}</p>
      </details>
      <!-- Conflicts -->
      <div v-if="preview && preview.conflicts.length > 0" class="preview-warnings">
        <div class="warning-header">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5"/>
            <path d="M12 8v4M12 16h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
          <span>{{ $t('migration.conflictsDetected', { count: preview.conflicts.length }) }}</span>
        </div>
        <div v-for="(conflict, i) in preview.conflicts.slice(0, 5)" :key="i" class="warning-item text-xs">
          {{ conflict }}
        </div>
        <div v-if="preview.conflicts.length > 5" class="warning-more text-xs text-tertiary">
          {{ $t('migration.andMore', { count: preview.conflicts.length - 5 }) }}
        </div>
      </div>

      <!-- Operations List -->
      <div v-if="preview && preview.operations.length > 0" class="preview-list glass-panel">
        <div class="preview-list-header text-xs text-tertiary">
          <span>{{ $t('migration.package_header') }}</span>
          <span>{{ $t('migration.type_header') }}</span>
          <span>{{ $t('migration.size_header') }}</span>
          <span>{{ $t('migration.destination') }}</span>
        </div>
        <div
          v-for="op in preview.operations.slice(0, 50)"
          :key="op.source"
          :class="['preview-row', { 'preview-conflict': op.conflict !== 'none' }]"
        >
          <span class="preview-cell name" :title="op.source">{{ fileName(op.source) }}</span>
          <span class="preview-cell type">{{ op.resource_type }}</span>
          <span class="preview-cell size">{{ formatSize(op.size_bytes) }}</span>
          <span class="preview-cell dest text-xs text-tertiary">{{ op.destination }}</span>
        </div>
        <div v-if="preview.operations.length > 50" class="preview-more text-xs text-tertiary">
          {{ $t('migration.andMoreFiles', { count: preview.operations.length - 50 }) }}
        </div>
      </div>

      <!-- Empty State -->
      <div v-else-if="preview && preview.operations.length === 0" class="preview-empty">
        <EmptyState
          icon="M4 7V4a2 2 0 0 1 2-2h8.5L20 7.5V20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3M2 12h16M6 9v6"
          :title="$t('migration.noOperations')"
          :description="$t('migration.noOperationsDesc')"
        />
      </div>

      <div v-if="!preview" class="preview-loading">
        <div class="dep-spinner" />
        <p class="text-sm text-secondary">{{ $t('migration.computingPlan') }}</p>
      </div>


    </div>

    <!-- Step 3: Execute -->
    <div v-else-if="currentStep === 3" class="step-content">
      <h2 class="step-title">
        {{ migrationResult ? $t('migration.migrationComplete') : $t('migration.executing') }}
      </h2>

      <!-- Progress Section (Spinner shown only during processing, progress bar always shown) -->
      <div class="exec-processing">
        <div v-if="!migrationResult" class="exec-spinner" />
        <p v-if="!migrationResult" class="text-sm text-secondary">{{ $t('migration.movingFiles') }}</p>
        
        <div v-if="migrationProgress || migrationResult" class="exec-progress">
          <div class="exec-progress-meta">
            <span>
              {{ migrationResult ? (migrationResult.completed + migrationResult.failed) : (migrationProgress ? (migrationProgress.completed + migrationProgress.failed) : 0) }} 
              / 
              {{ migrationResult ? migrationResult.total : (migrationProgress ? migrationProgress.total : 0) }}
            </span>
            <span v-if="!migrationResult">{{ $t('migration.remainingFiles', { count: remainingFiles }) }}</span>
            <span v-else class="text-success">{{ $t('migration.completed') }}</span>
          </div>
          <div class="exec-progress-track">
            <div class="exec-progress-fill" :style="{ width: `${migrationResult ? 100 : migrationProgressPercent}%` }" />
          </div>
          <div v-if="migrationProgress?.current_package_id && !migrationResult" class="exec-progress-file">
            {{ migrationProgress.current_package_id }}
          </div>
        </div>
      </div>

      <!-- Result Summary (shown only after completion) -->
      <div v-if="migrationResult" class="exec-result">
        <div class="exec-summary glass-panel">
          <div class="exec-stat">
            <span class="exec-stat-value exec-ok">{{ migrationResult.completed }}</span>
            <span class="exec-stat-label text-xs text-tertiary">{{ $t('migration.completed') }}</span>
          </div>
          <div class="exec-stat">
            <span :class="['exec-stat-value', migrationResult.failed > 0 ? 'exec-err' : 'exec-ok']">
              {{ migrationResult.failed }}
            </span>
            <span class="exec-stat-label text-xs text-tertiary">{{ $t('migration.failed') }}</span>
          </div>
          <div class="exec-stat">
            <span class="exec-stat-value">{{ migrationResult.total }}</span>
            <span class="exec-stat-label text-xs text-tertiary">{{ $t('migration.total') }}</span>
          </div>
        </div>

        <p class="text-sm text-secondary">{{ t('migration.removedDirectories', { count: migrationResult.removed_directories }) }}</p>
        <!-- Errors -->
        <div v-if="migrationResult.errors.length > 0" class="exec-errors glass-panel">
          <h4 class="exec-errors-title text-sm">{{ $t('migration.errors') }}</h4>
          <div v-for="(err, i) in migrationResult.errors" :key="i" class="exec-error-item text-xs">
            {{ err }}
          </div>
        </div>
      </div>


    </div>

    <div v-if="restoreAllMessage" :class="['restore-all-message', restoreAllFailed ? 'error' : 'ok']">
      {{ restoreAllMessage }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import EmptyState from '@/components/common/EmptyState.vue'

interface MigrationOperation {
  package_id: string
  source: string
  destination: string
  action: string
  size_bytes: number
  resource_type: string
  conflict: string
}

interface MigrationPreview {
  plan_id: string
  total_files: number
  skipped: number
  warnings: string[]
  cleanup_empty_dirs: boolean
  total_operations: number
  total_size_bytes: number
  operations: MigrationOperation[]
  conflicts: string[]
}

interface MigrationResult {
  task_id: string
  completed: number
  failed: number
  total: number
  errors: string[]
  rollback_available: boolean
  removed_directories: number
}

interface MigrationRollbackResult {
  task_id: string
  restored: number
  failed: number
  errors: string[]
}

interface MigrationProgress {
  task_id: string
  phase: string
  completed: number
  failed: number
  total: number
  current_package_id: string | null
}

interface OnDemandOperationResult {
  completed: number
  failed: number
  skipped: number
  total: number
  errors: string[]
}

const { t, locale } = useI18n()
const appStore = useAppStore()
const { vamRootPath, installContext } = storeToRefs(appStore)

const currentStep = ref(0)
const selectedMode = ref<string | null>('by_type')
const preview = ref<MigrationPreview | null>(null)
const migrationResult = ref<MigrationResult | null>(null)
const isRollingBack = ref(false)
const isRestoringAll = ref(false)
const migrationProgress = ref<MigrationProgress | null>(null)
const restoreAllMessage = ref('')
const restoreAllFailed = ref(false)
let unlistenMigrationProgress: (() => void) | null = null

const config = ref({
  sourceDir: '',
  targetKind: 'real_addon' as 'real_addon' | 'managed_library',
  action: 'move' as 'copy' | 'move',
})

const steps = computed(() => [
  t('migration.stepMode'),
  t('migration.stepConfigure'),
  t('migration.stepPreview'),
  t('migration.stepExecute'),
])

const modes = computed(() => [
  {
    id: 'flatten',
    title: t('migration.modeFlatten'),
    description: t('migration.modeFlattenDesc'),
    icon: 'M3 7h18M12 3v14m-5-5 5 5 5-5M4 21h16',
    color: '#5b8def',
  },
  {
    id: 'by_type',
    title: t('migration.modeByType'),
    description: t('migration.modeByTypeDesc'),
    icon: 'M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z',
    color: '#6e6bf0',
  },
  {
    id: 'by_creator',
    title: t('migration.modeByCreator'),
    description: t('migration.modeByCreatorDesc'),
    icon: 'M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75',
    color: '#5b8def',
  },
  {
    id: 'by_scene',
    title: t('migration.modeByScene'),
    description: t('migration.modeBySceneDesc'),
    icon: 'M2 4h20v16H2zM2 9h20M5 6.5h0M7.5 6.5h0M10 6.5h0',
    color: '#f59e0b',
  },
  {
    id: 'custom',
    title: t('migration.modeByCustom'),
    description: t('migration.modeByCustomDesc'),
    icon: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z',
    color: '#3ecf8e',
  },
])

const realAddonDir = computed(() => (
  installContext.value?.realAddonDir || (vamRootPath.value ? `${vamRootPath.value}/AddonPackages` : '')
))

const managedLibraryDir = computed(() => (
  installContext.value?.managedLibraryDir || (vamRootPath.value ? `${vamRootPath.value}/VAMBoxLibrary/AddonPackages` : '')
))

const selectedTargetDir = computed(() => (
  config.value.targetKind === 'real_addon' ? realAddonDir.value : managedLibraryDir.value
))

const targetOptions = computed(() => [
  {
    id: 'managed_library' as const,
    label: t('migration.targetManagedLibrary'),
  },
  {
    id: 'real_addon' as const,
    label: t('migration.targetRealAddon'),
  },
])

const remainingFiles = computed(() => {
  if (!migrationProgress.value) return 0
  return Math.max(0, migrationProgress.value.total - migrationProgress.value.completed - migrationProgress.value.failed)
})

const migrationProgressPercent = computed(() => {
  if (!migrationProgress.value || migrationProgress.value.total === 0) return 0
  return Math.min(100, Math.round((migrationProgress.value.completed + migrationProgress.value.failed) / migrationProgress.value.total * 100))
})

onMounted(async () => {
  unlistenMigrationProgress = await listen<MigrationProgress>('migration-progress', (event) => {
    migrationProgress.value = event.payload
  })
})

onUnmounted(() => {
  if (unlistenMigrationProgress) {
    unlistenMigrationProgress()
    unlistenMigrationProgress = null
  }
})

async function handlePreview() {
  if (!selectedMode.value) return
  preview.value = null
  currentStep.value = 2

  const sourceDir = config.value.sourceDir || realAddonDir.value
  const targetDir = selectedTargetDir.value

  try {
    preview.value = await invoke<MigrationPreview>('preview_migration', {
      config: {
        mode: selectedMode.value,
        source_dir: sourceDir,
        target_dir: targetDir,
        dry_run: config.value.action === 'copy',
        locale: locale.value,
      },
    })
  } catch (err) {
    preview.value = { plan_id: '', total_files: 0, skipped: 0, warnings: [], cleanup_empty_dirs: false, total_operations: 0, total_size_bytes: 0, operations: [], conflicts: [String(err)] }
  }
}

async function handleExecute() {
  if (!preview.value?.plan_id) return
  currentStep.value = 3
  migrationProgress.value = {
    task_id: '',
    phase: 'executing',
    completed: 0,
    failed: 0,
    total: preview.value.operations.length,
    current_package_id: null,
  }


  try {
    migrationResult.value = await invoke<MigrationResult>('execute_migration', {
      planId: preview.value.plan_id,
    })
    if (migrationResult.value.completed > 0 && config.value.targetKind === 'managed_library' && managedLibraryDir.value) {
      try {
        await appStore.setManagedSettings(true, managedLibraryDir.value)
      } catch (err) {
        migrationResult.value.errors.push(t('migration.settingsSaveFailed', { error: String(err) }))
      }
    }
  } catch (err) {
    migrationResult.value = {
      task_id: '',
      completed: 0,
      failed: 1,
      total: preview.value.operations.length,
      errors: [String(err)],
      removed_directories: 0,
      rollback_available: false,
    }
  }
}

async function handleRollback() {
  if (!migrationResult.value?.task_id || isRollingBack.value) return
  isRollingBack.value = true
  try {
    const result = await invoke<MigrationRollbackResult>('rollback_migration', {
      taskId: migrationResult.value.task_id,
    })
    migrationResult.value.errors = result.errors
    migrationResult.value.completed = result.restored
    migrationResult.value.failed = result.failed
    migrationResult.value.total = result.restored + result.failed
    migrationResult.value.rollback_available = result.failed > 0
  } catch (err: any) {
    migrationResult.value.errors = [String(err)]
  } finally {
    isRollingBack.value = false
  }
}

async function handleRestoreAll() {
  if (isRestoringAll.value) return
  isRestoringAll.value = true
  restoreAllMessage.value = ''
  restoreAllFailed.value = false
  try {
    const migrationRollback = await invoke<MigrationRollbackResult>('rollback_all_migrations')
    let restored = migrationRollback.restored
    let failed = migrationRollback.failed
    const errors = [...migrationRollback.errors]

    if (vamRootPath.value) {
      const libraryRestore = await invoke<OnDemandOperationResult>('restore_on_demand_library', {
        vamRoot: vamRootPath.value,
      })
      restored += libraryRestore.completed
      failed += libraryRestore.failed
      errors.push(...libraryRestore.errors)
    }

    restoreAllFailed.value = failed > 0
    restoreAllMessage.value = t('migration.restoreAllResult', {
      restored,
      failed,
      errors: errors.length,
    })
    await appStore.refreshInstallContext()
  } catch (err: any) {
    restoreAllFailed.value = true
    restoreAllMessage.value = String(err)
  } finally {
    isRestoringAll.value = false
  }
}

function resetWizard() {
  currentStep.value = 0
  selectedMode.value = 'by_type'
  preview.value = null
  migrationResult.value = null
  migrationProgress.value = null
}

function fileName(path: string) { return path.split(/[\\/]/).pop() || path }

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
}
</script>

<style scoped>
.migration-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.migration-header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-6);
  border-radius: var(--radius-md);
  flex-wrap: wrap;
  gap: var(--space-4);
}

.step-indicator {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.step-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.step {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.step-connector {
  flex: 1;
  height: 1px;
  margin: 0 0 0 var(--space-3);
  background: var(--border-subtle);
  min-width: 30px;
}

.step-number {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--bg-hover);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  flex-shrink: 0;
  transition: background var(--duration-base) var(--ease), color var(--duration-base) var(--ease);
}

.step.active .step-number { background: var(--accent-primary); color: white; }
.step.completed .step-number { background: var(--color-success); color: white; }

.step-label {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-tertiary);
  white-space: nowrap;
  transition: color var(--duration-base) var(--ease);
}

.step.active .step-label { color: var(--text-primary); }
.step.completed .step-label { color: var(--text-secondary); }

.step-content { padding: var(--space-2) 0; }

.step-title {
  font-size: var(--text-xl);
  font-weight: var(--font-bold);
  margin-bottom: var(--space-2);
}

.step-description {
  font-size: var(--text-sm);
  margin-bottom: var(--space-6);
}

/* ── Mode Grid ────────────────────────────────────────────── */
.mode-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-4);
}

@media (max-width: 900px) { .mode-grid { grid-template-columns: 1fr; } }

.mode-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-6);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  text-align: left;
  cursor: pointer;
  transition: transform var(--duration-base) var(--ease), box-shadow var(--duration-base) var(--ease), border-color var(--duration-base) var(--ease);
}

.mode-card:hover { transform: translateY(-2px); box-shadow: var(--glass-shadow-lg); border-color: var(--border-strong); }
.mode-card.selected { border-color: var(--accent-primary); box-shadow: 0 0 0 1px var(--accent-primary), var(--glass-shadow-lg); }

.mode-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--mode-color) 12%, transparent);
  color: var(--mode-color);
}

.mode-title { font-size: var(--text-md); font-weight: var(--font-semibold); color: var(--text-primary); }
.mode-description { font-size: var(--text-sm); color: var(--text-secondary); line-height: var(--leading-relaxed); }

.selected-indicator { position: absolute; top: var(--space-4); right: var(--space-4); color: var(--accent-primary); }

/* ── Step Footer ──────────────────────────────────────────── */
.step-footer {
  display: flex;
  justify-content: space-between;
  margin-top: var(--space-6);
}

.step-back-btn, .step-next-btn, .step-execute-btn, .step-rollback-btn {
  display: inline-flex;
  align-items: center;
  padding: var(--space-2) var(--space-5);
  height: 36px;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  transition: opacity var(--duration-fast) var(--ease), transform var(--duration-fast) var(--ease);
}

.step-back-btn { background: var(--bg-elevated); color: var(--text-secondary); border: 1px solid var(--border-subtle); }
.step-back-btn:hover { color: var(--text-primary); }

.step-next-btn { background: var(--accent-gradient); color: white; }
.step-next-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.step-next-btn:not(:disabled):hover { opacity: 0.9; transform: translateY(-1px); }

.step-execute-btn { background: var(--color-success); color: white; }
.step-execute-btn:hover { opacity: 0.9; transform: translateY(-1px); }

.step-rollback-btn {
  background: rgba(248, 113, 113, 0.12);
  color: var(--color-error);
  border: 1px solid rgba(248, 113, 113, 0.25);
}
.step-rollback-btn:hover:not(:disabled) { background: rgba(248, 113, 113, 0.18); transform: translateY(-1px); }
.step-rollback-btn:disabled { opacity: 0.5; cursor: not-allowed; }

/* ── Config Form ──────────────────────────────────────────── */
.config-form { display: flex; flex-direction: column; gap: var(--space-5); max-width: 560px; }

.config-field { display: flex; flex-direction: column; gap: var(--space-2); }

.config-label { font-weight: var(--font-semibold); color: var(--text-primary); }

.config-input-row { display: flex; flex-direction: column; gap: var(--space-1); }

.config-input {
  height: 36px;
  padding: 0 var(--space-3);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-sm);
  transition: border-color var(--duration-fast) var(--ease);
}

.config-input:focus { border-color: var(--accent-primary); outline: none; }

.target-option-row {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.selected-target {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  min-height: 36px;
  padding: 0 var(--space-3);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.selected-target-path {
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.config-toggle-row { display: flex; gap: var(--space-2); }

.config-toggle {
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  transition: background var(--duration-fast) var(--ease), color var(--duration-fast) var(--ease), border-color var(--duration-fast) var(--ease);
}

.config-toggle.active { background: rgba(110, 107, 240, 0.12); color: var(--accent-primary); border-color: var(--accent-primary); }

/* ── Preview ──────────────────────────────────────────────── */
.preview-warnings {
  background: rgba(251, 191, 36, 0.08);
  border: 1px solid rgba(251, 191, 36, 0.2);
  border-radius: var(--radius-md);
  padding: var(--space-3);
  margin-bottom: var(--space-4);
}

.warning-header { display: flex; align-items: center; gap: var(--space-2); color: var(--color-warning); font-size: var(--text-sm); font-weight: var(--font-semibold); margin-bottom: var(--space-2); }

.warning-item { color: var(--color-warning); padding: 2px 0; }
.warning-more { padding-top: var(--space-1); }

.preview-list { overflow: hidden; margin-bottom: var(--space-4); }

.preview-list-header {
  display: grid;
  grid-template-columns: 2fr 1fr 80px 2fr;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.preview-row {
  display: grid;
  grid-template-columns: 2fr 1fr 80px 2fr;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  font-size: var(--text-sm);
  color: var(--text-primary);
  align-items: center;
}

.preview-row.preview-conflict { background: rgba(251, 191, 36, 0.05); }

.preview-cell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.preview-cell.name { font-weight: var(--font-medium); }
.preview-cell.size { font-variant-numeric: tabular-nums; }

.preview-more { padding: var(--space-2) var(--space-4); }
.preview-empty { margin: var(--space-10) 0; }

.preview-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8);
}

/* ── Execute ──────────────────────────────────────────────── */
.exec-processing {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-10);
}

.exec-spinner, .dep-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--accent-primary);
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }

.exec-progress {
  width: min(520px, 100%);
}

.exec-progress-meta {
  display: flex;
  justify-content: space-between;
  margin-bottom: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.exec-progress-track {
  height: 8px;
  overflow: hidden;
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
}

.exec-progress-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent-gradient);
  transition: width var(--duration-base) var(--ease);
}

.exec-progress-file {
  margin-top: var(--space-2);
  overflow: hidden;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.exec-summary {
  display: flex;
  justify-content: center;
  gap: var(--space-10);
  padding: var(--space-6);
}

.exec-stat { display: flex; flex-direction: column; align-items: center; gap: var(--space-1); }

.exec-stat-value { font-size: var(--font-size-2xl); font-weight: var(--font-bold); font-variant-numeric: tabular-nums; color: var(--text-primary); }

.exec-stat-value.exec-ok { color: var(--color-success); }
.exec-stat-value.exec-err { color: var(--color-error); }

.exec-errors { margin-top: var(--space-4); padding: var(--space-4); }

.exec-errors-title { font-weight: var(--font-semibold); color: var(--color-error); margin-bottom: var(--space-3); }

.exec-error-item {
  padding: var(--space-2) 0;
  color: var(--color-error);
  opacity: 0.8;
}

.exec-error-item + .exec-error-item { border-top: 1px solid var(--border-subtle); }

.restore-all-message {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
}

.restore-all-message.ok {
  color: var(--color-success);
  background: var(--color-success-bg);
}

.restore-all-message.error {
  color: var(--color-error);
  background: var(--color-error-bg);
}
</style>
