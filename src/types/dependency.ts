/** Dependency information */
export interface DependencyInfo {
  package_id: string;
  status: 'installed' | 'missing' | 'version_mismatch';
  installed_version: number | null;
  required_version: string;
  size_bytes: number | null;
}

/** Dependency graph data for visualization */
export interface DependencyGraphData {
  nodes: DependencyNode[];
  edges: DependencyEdge[];
}

export interface DependencyNode {
  id: string;
  creator: string;
  name: string;
  version: number;
  resource_type: string;
  size_bytes: number;
  dependents_count: number;
  dependencies_count: number;
  status: string;
}

export interface DependencyEdge {
  source: string;
  target: string;
  is_transitive: boolean;
}
