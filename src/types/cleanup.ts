export interface CleanupMark { package_id: string; pending: boolean; protected: boolean }
export interface CleanupReason { code: string; source: string }
/** `invalid_download` | `wrong_format` | `damaged`: the archive cannot be opened at all. */
export type CleanupIssue = 'invalid_download' | 'wrong_format' | 'damaged'
export interface CleanupItem { package_id: string; file_count: number; size_bytes: number; content_count: number; attached: boolean; reasons: CleanupReason[]; issue?: CleanupIssue | null; invalid_name?: boolean }
export interface CleanupPreview { plan_id: string; items: CleanupItem[]; blocked: CleanupItem[]; retained: CleanupItem[]; warnings: string[]; suggested?: CleanupItem[]; suggested_bytes?: number; requires_confirmation: boolean; total_bytes: number; total_files: number; quick: boolean }
export interface CleanupFile { package_id: string; original: { path: string; size: number; modified: string }; hash: string; trash_path: string; status: string }
export interface CleanupBatch { id: string; created_at: string; expires_at: string; status: string; files: CleanupFile[]; errors: string[] }
export interface CleanupPolicy { retention_days: number; auto_purge: boolean; background_index: boolean }
export interface CleanupIndexStatus { state: 'idle' | 'waiting' | 'paused_game' | 'running' | 'ready' | 'disabled' | 'error'; stage: string; current: number; total: number; message: string | null; finished_at: string | null }
export interface CleanupProgress { request_id: string; stage: string; current: number; total: number }
/** Only packages with something to report are listed. */
export interface PackageHealth { package_id: string; issue: CleanupIssue | null; invalid_name?: boolean; copies: number; unused: boolean; standalone?: boolean }
export interface CleanupHealthReport { packages: PackageHealth[]; checked_at: string }
