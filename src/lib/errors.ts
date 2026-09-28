/**
 * One way to read an error for a person. Tauri rejects commands with the
 * Rust `String` (or a serialized struct), tests throw plain strings, and
 * browser APIs throw `Error`s; `String(err)` renders the second kind as
 * `[object Object]`. This reads each shape for its message.
 */
export function describeError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message || err.name;
  if (typeof err === "object" && err !== null) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === "string" && message) return message;
    try {
      return JSON.stringify(err);
    } catch {
      /* fall through */
    }
  }
  return String(err);
}
