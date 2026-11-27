import en from "./en.json";
import vi from "./vi.json";

export type Lang = "en" | "vi";

const dictionaries: Record<Lang, unknown> = { en, vi };

function lookup(dict: unknown, key: string): string | undefined {
  const value = key.split(".").reduce<unknown>((node, part) => {
    if (node && typeof node === "object" && part in node) {
      return (node as Record<string, unknown>)[part];
    }
    return undefined;
  }, dict);
  return typeof value === "string" ? value : undefined;
}

/** Translate a dotted key, filling `{name}` placeholders. */
export function translate(
  lang: Lang,
  key: string,
  vars?: Record<string, string | number>,
): string {
  const template = lookup(dictionaries[lang], key) ?? lookup(en, key) ?? key;
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_, name: string) =>
    name in vars ? String(vars[name]) : `{${name}}`,
  );
}
