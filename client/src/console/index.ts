// The Console design system. Every export is a building block: nothing here
// fetches, routes, or knows what a screen is.
//
// Domain shapes are the wire's. Anything describing a machine, a conversation, a
// question or a part is imported from $bindings rather than redefined here, so a
// change in the Rust common crate surfaces as a type error instead of drift.

export { default as AttachChip } from "./components/AttachChip/AttachChip.svelte";
export { default as BrailleSpinner } from "./components/BrailleSpinner/BrailleSpinner.svelte";
export { default as Button } from "./components/Button/Button.svelte";
export { default as Chip } from "./components/Chip/Chip.svelte";
export { default as CodeSlots } from "./components/CodeSlots/CodeSlots.svelte";
export { default as Composer } from "./components/Composer/Composer.svelte";
export { default as ConnDot } from "./components/ConnDot/ConnDot.svelte";
export { default as ContextBar } from "./components/ContextBar/ContextBar.svelte";
export { default as DiffView } from "./components/DiffView/DiffView.svelte";
export { default as Drawer } from "./components/Drawer/Drawer.svelte";
export { default as EmptyState } from "./components/EmptyState/EmptyState.svelte";
export { default as FileCard } from "./components/FileCard/FileCard.svelte";
export { default as FilePreview } from "./components/FilePreview/FilePreview.svelte";
export { default as FileViewer } from "./components/FileViewer/FileViewer.svelte";
export { default as Icon } from "./components/Icon/Icon.svelte";
export { default as KeyBar } from "./components/KeyBar/KeyBar.svelte";
export { default as Label } from "./components/Label/Label.svelte";
export { default as Markdown } from "./components/Markdown/Markdown.svelte";
export { default as NavBar } from "./components/NavBar/NavBar.svelte";
export { default as PaneMap } from "./components/PaneMap/PaneMap.svelte";
export { default as PartView } from "./components/PartView/PartView.svelte";
export { default as QuestionCard } from "./components/QuestionCard/QuestionCard.svelte";
export { default as QuestionFlow } from "./components/QuestionFlow/QuestionFlow.svelte";
export { default as StatusGlyph } from "./components/StatusGlyph/StatusGlyph.svelte";
export { default as StatusLine } from "./components/StatusLine/StatusLine.svelte";
export { default as TableView } from "./components/TableView/TableView.svelte";
export { default as TabStrip } from "./components/TabStrip/TabStrip.svelte";
export { default as TerminalPane } from "./components/TerminalPane/TerminalPane.svelte";
export { default as TerminalView } from "./components/TerminalView/TerminalView.svelte";
export { default as ThinkingRow } from "./components/ThinkingRow/ThinkingRow.svelte";
export { default as Timeline } from "./components/Timeline/Timeline.svelte";
export { default as TodoList } from "./components/TodoList/TodoList.svelte";
export { default as Toggle } from "./components/Toggle/Toggle.svelte";
export { default as ToolFold } from "./components/ToolFold/ToolFold.svelte";
export { default as Tree } from "./components/Tree/Tree.svelte";
export { default as TreeNode } from "./components/TreeNode/TreeNode.svelte";
export { default as ViewToggle } from "./components/ViewToggle/ViewToggle.svelte";
export { default as TreeTwig } from "./components/TreeTwig/TreeTwig.svelte";
export { default as Turn } from "./components/Turn/Turn.svelte";

export type { AttachChipProps } from "./components/AttachChip/AttachChip.types";
export type { BrailleSpinnerProps } from "./components/BrailleSpinner/BrailleSpinner.types";
export type { ButtonProps, ButtonVariant } from "./components/Button/Button.types";
export type { ChipProps } from "./components/Chip/Chip.types";
export type { CodeSlotsProps } from "./components/CodeSlots/CodeSlots.types";
export type { Attachment, ComposerProps } from "./components/Composer/Composer.types";
export type { ConnDotProps } from "./components/ConnDot/ConnDot.types";
export type { ContextBarProps } from "./components/ContextBar/ContextBar.types";
export type { DiffViewProps } from "./components/DiffView/DiffView.types";
export type { DrawerProps } from "./components/Drawer/Drawer.types";
export type { EmptyStateProps } from "./components/EmptyState/EmptyState.types";
export type { FileCardProps } from "./components/FileCard/FileCard.types";
export type { FilePreviewProps } from "./components/FilePreview/FilePreview.types";
export type { FileViewerProps } from "./components/FileViewer/FileViewer.types";
export type { IconName, IconProps } from "./components/Icon/Icon.types";
export type { KeyBarProps, KeyCap } from "./components/KeyBar/KeyBar.types";
export { DEFAULT_KEYS, MOD } from "./components/KeyBar/KeyBar.types";
export type { LabelKind, LabelProps } from "./components/Label/Label.types";
export type { MarkdownProps } from "./components/Markdown/Markdown.types";
export type { NavBarProps } from "./components/NavBar/NavBar.types";
export type { PaneBox, PaneMapProps } from "./components/PaneMap/PaneMap.types";
export type { PartViewProps } from "./components/PartView/PartView.types";
export type { QuestionCardProps } from "./components/QuestionCard/QuestionCard.types";
export type { QuestionFlowProps } from "./components/QuestionFlow/QuestionFlow.types";
export type { StatusGlyphProps } from "./components/StatusGlyph/StatusGlyph.types";
export type { StatusLineProps } from "./components/StatusLine/StatusLine.types";
export type { TableViewProps } from "./components/TableView/TableView.types";
export type { TabStripProps } from "./components/TabStrip/TabStrip.types";
export type { TerminalPaneProps } from "./components/TerminalPane/TerminalPane.types";
export type { TerminalViewProps } from "./components/TerminalView/TerminalView.types";
export type { ThinkingRowProps } from "./components/ThinkingRow/ThinkingRow.types";
export type { TimelineProps } from "./components/Timeline/Timeline.types";
export type { TodoListProps } from "./components/TodoList/TodoList.types";
export type { ToggleProps } from "./components/Toggle/Toggle.types";
export type { ToolFoldProps } from "./components/ToolFold/ToolFold.types";
export type { TreeProps } from "./components/Tree/Tree.types";
export type { TreeNodeProps } from "./components/TreeNode/TreeNode.types";
export type { TreeTwigProps } from "./components/TreeTwig/TreeTwig.types";
export type { TurnProps } from "./components/Turn/Turn.types";
export type { ViewToggleProps, WorkspaceView } from "./components/ViewToggle/ViewToggle.types";

// Client-side shapes. Everything else a component speaks comes from $bindings.
export type { DrawerHeight, GlyphState } from "./types/state";
export { isBlocking } from "./types/state";
export type { Draft } from "./types/questions";
export { EMPTY_DRAFT, isComplete, toAnswer, toAnswers } from "./types/questions";
export type { AgentStats } from "./types/stats";
export type { FileMeta, PreviewKind } from "./types/files";

export { fileExtension, formatBytes, formatDuration, formatTokens } from "./lib/format";
export { inlineText, parseInline, parseMarkdown, plainText } from "./lib/markdown";
export type { Block, Inline } from "./lib/markdown";
export { autogrow, growHeight } from "./lib/autogrow";
export { isPreviewable, previewKind, PREVIEW_BYTES } from "./lib/preview";
export { ATTR, charWidth, TerminalGrid, toRuns } from "./lib/terminal";
export type { Cell, Run } from "./lib/terminal";
export { cssColor, indexedColor, runStyle } from "./lib/terminal-color";
export type { RunStyle } from "./lib/terminal-color";
export { token, TOKENS, type TokenName } from "./tokens/tokens";
