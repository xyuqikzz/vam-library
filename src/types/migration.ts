/** Migration task definition */
export interface MigrationTask {
  id: string;
  mode: 'by_type' | 'by_creator' | 'by_scene' | 'custom';
  source_dir: string;
  rules: MigrationRule[];
  operations: MigrationOperation[];
  status: 'preview' | 'executing' | 'completed' | 'failed' | 'rolled_back';
  progress: number;
}

export interface MigrationRule {
  match: 'type' | 'creator' | 'scene_dep' | 'regex';
  pattern?: string;
  target_dir: string;
}

export interface MigrationOperation {
  source: string;
  destination: string;
  action: 'move' | 'copy';
  size_bytes: number;
  conflict: 'none' | 'overwrite' | 'skip' | 'rename';
}

/** Duplicate resource group */
export interface DuplicateGroup {
  strategy: 'exact' | 'path' | 'version';
  resource_path: string;
  hash?: string;
  total_wasted_bytes: number;
  instances: DuplicateInstance[];
}

export interface DuplicateInstance {
  package_id: string;
  file_path: string;
  size_bytes: number;
  is_recommended_keep: boolean;
}
