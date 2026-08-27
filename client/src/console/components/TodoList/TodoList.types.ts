import type { TodoItem } from "$bindings/TodoItem";

export interface TodoListProps {
  items: TodoItem[];
  /** Heading above the list. */
  label?: string;
}
