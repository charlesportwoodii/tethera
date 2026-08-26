// The Console design system. Every export is a building block: nothing here
// fetches, routes, or knows what a screen is.

export { default as AskBlock } from "./components/AskBlock/AskBlock.svelte";
export { default as Button } from "./components/Button/Button.svelte";
export { default as Chip } from "./components/Chip/Chip.svelte";
export { default as CodeSlots } from "./components/CodeSlots/CodeSlots.svelte";
export { default as Composer } from "./components/Composer/Composer.svelte";
export { default as ConnDot } from "./components/ConnDot/ConnDot.svelte";
export { default as Drawer } from "./components/Drawer/Drawer.svelte";
export { default as FileCard } from "./components/FileCard/FileCard.svelte";
export { default as Icon } from "./components/Icon/Icon.svelte";
export { default as KeyBar } from "./components/KeyBar/KeyBar.svelte";
export { default as Label } from "./components/Label/Label.svelte";
export { default as NavBar } from "./components/NavBar/NavBar.svelte";
export { default as PartView } from "./components/PartView/PartView.svelte";
export { default as StatusGlyph } from "./components/StatusGlyph/StatusGlyph.svelte";
export { default as TabStrip } from "./components/TabStrip/TabStrip.svelte";
export { default as TerminalView } from "./components/TerminalView/TerminalView.svelte";
export { default as Timeline } from "./components/Timeline/Timeline.svelte";
export { default as ToolFold } from "./components/ToolFold/ToolFold.svelte";
export { default as Tree } from "./components/Tree/Tree.svelte";
export { default as TreeNode } from "./components/TreeNode/TreeNode.svelte";
export { default as TreeTwig } from "./components/TreeTwig/TreeTwig.svelte";
export { default as Turn } from "./components/Turn/Turn.svelte";
export { default as Toggle } from "./components/Toggle/Toggle.svelte";

export type { AskBlockProps, AskOption } from "./components/AskBlock/AskBlock.types";
export type { ButtonProps, ButtonVariant } from "./components/Button/Button.types";
export type { ChipProps } from "./components/Chip/Chip.types";
export type { CodeSlotsProps } from "./components/CodeSlots/CodeSlots.types";
export type { ComposerProps } from "./components/Composer/Composer.types";
export type { ConnDotProps } from "./components/ConnDot/ConnDot.types";
export type { DrawerProps } from "./components/Drawer/Drawer.types";
export type { FileCardProps } from "./components/FileCard/FileCard.types";
export type { IconName, IconProps } from "./components/Icon/Icon.types";
export type { KeyBarProps } from "./components/KeyBar/KeyBar.types";
export { DEFAULT_KEYS } from "./components/KeyBar/KeyBar.types";
export type { LabelKind, LabelProps } from "./components/Label/Label.types";
export type { NavBarProps } from "./components/NavBar/NavBar.types";
export type { PartViewProps } from "./components/PartView/PartView.types";
export type { StatusGlyphProps } from "./components/StatusGlyph/StatusGlyph.types";
export type { PaneTab, TabStripProps } from "./components/TabStrip/TabStrip.types";
export type {
  TerminalViewProps,
  TermLine,
  TermTone,
} from "./components/TerminalView/TerminalView.types";
export type { TimelineProps } from "./components/Timeline/Timeline.types";
export type { ToolFoldProps } from "./components/ToolFold/ToolFold.types";
export type { TreeProps } from "./components/Tree/Tree.types";
export type { TreeNodeProps } from "./components/TreeNode/TreeNode.types";
export type { TreeTwigProps } from "./components/TreeTwig/TreeTwig.types";
export type { TurnProps } from "./components/Turn/Turn.types";
export type { ToggleProps } from "./components/Toggle/Toggle.types";

export type {
  DrawerHeight,
  GlyphState,
  LinkState,
  TurnRole,
} from "./types/state";
export { isBlocking } from "./types/state";
export { token, TOKENS, type TokenName } from "./tokens/tokens";
