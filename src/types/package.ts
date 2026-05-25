import type { ResourceType } from './resource';

/** Represents a parsed .var package from the backend */
export interface VarPackage {
  id: string;
  creator: string;
  name: string;
  version: number;
  file_path: string;
  size_bytes: number;
  meta: PackageMeta | null;
  resource_types: ResourceType[];
  scan_time: string;
}

/** 资源展示组件中使用的展示项类型（与 VarPackageSummary 对齐后端最新返回字段） */
export interface PackageDisplayItem {
  id: string;
  creator: string;
  name: string;
  version: number;
  file_path: string;
  size_bytes: number;
  resource_types: string[];
  dependency_count: number;
  dependents_count: number;
  content_count: number;
  created_time: string;
  scan_time: string;
  tags: string[];
}

export interface PackageImageEntry {
  path: string;
  size_bytes: number;
}

export interface PackageFolderEntry {
  path: string;
}

/** Summary for list/grid views */
export interface VarPackageSummary {
  id: string;
  creator: string;
  name: string;
  version: number;
  file_path: string;
  size_bytes: number;
  resource_types: ResourceType[];
  /** 此包依赖了多少个其他包 */
  dependency_count: number;
  /** 有多少个其他包依赖此包 */
  dependents_count: number;
  /** 包内文件数量 */
  content_count: number;
  /** ISO 时间戳 - 文件创建时间 */
  created_time: string;
  /** ISO 时间戳 - 入库时间 */
  scan_time: string;
  tags: string[];
}

/** Parsed meta.json from a .var file */
export interface PackageMeta {
  license_type: string;
  creator_name: string;
  package_name: string;
  description: string | null;
  credits: string | null;
  instructions: string | null;
  promotional_link: string | null;
  content_list: string[];
  dependencies: Record<string, unknown>;
  custom_options: Record<string, string> | null;
}

/** Filter options for package queries */
export interface PackageFilter {
  search: string;
  types: ResourceType[];
  creators: string[];
  sizeRange: [number, number] | null;
  sortBy: 'name' | 'size' | 'date' | 'creator' | 'version' | 'created' | 'imported';
  sortOrder: 'asc' | 'desc';
}

/** Dashboard aggregate statistics */
export interface DashboardStats {
  total_packages: number;
  total_size_bytes: number;
  scene_count: number;
  appearance_count: number;
  morph_count: number;
  plugin_count: number;
  missing_dependencies: number;
  duplicate_resources: number;
  orphaned_packages: number;
  corrupted_packages: number;
}

export interface CorruptedPackage {
  file_path: string;
  package_id: string;
  size_bytes: number;
  error: string;
}

