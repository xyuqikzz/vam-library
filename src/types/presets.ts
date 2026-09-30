// Order is the visible order in the preset browser; appearance is the default.
export const presetKinds = ['appearance', 'clothing', 'hair', 'morphs', 'skin', 'plugins', 'animation', 'pose'] as const
export type PresetKind = typeof presetKinds[number]
export type GameContentKind = PresetKind | 'scene'
