import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";

// Each test owns its DOM. Without this, a component from the previous test is
// still mounted and getByRole finds two of everything.
afterEach(cleanup);
