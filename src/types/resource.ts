/** Resource types matching the Rust backend */
export type ResourceType =
  | 'scene'
  | 'appearance'
  | 'morph'
  | 'clothing'
  | 'hair'
  | 'texture'
  | 'plugin'
  | 'asset'
  | 'sound'
  | 'other';

/** Maps resource types to display names */
export const RESOURCE_TYPE_LABELS: Record<ResourceType, string> = {
  scene: '场景',
  appearance: '外观',
  morph: '变形',
  clothing: '服装',
  hair: '头发',
  texture: '纹理',
  plugin: '插件',
  asset: '资产',
  sound: '音效',
  other: '其他',
};

/** Maps resource types to their badge variants */
export const RESOURCE_TYPE_VARIANTS: Record<ResourceType, string> = {
  scene: 'scene',
  appearance: 'appearance',
  morph: 'morph',
  clothing: 'clothing',
  hair: 'hair',
  texture: 'texture',
  plugin: 'plugin',
  asset: 'asset',
  sound: 'info',
  other: 'default',
};
