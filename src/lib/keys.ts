// Conversii între tastele din browser, codurile virtuale Windows și scurtăturile Tauri.

const VK_SPECIAL: Record<string, [number, string]> = {
  Space: [0x20, "Space"], Tab: [0x09, "Tab"], Enter: [0x0d, "Enter"],
  ShiftLeft: [0xa0, "Shift"], ShiftRight: [0xa1, "Shift dreapta"],
  ControlLeft: [0xa2, "Ctrl"], ControlRight: [0xa3, "Ctrl dreapta"],
  AltLeft: [0xa4, "Alt"], AltRight: [0xa5, "Alt dreapta"],
  CapsLock: [0x14, "Caps Lock"], Backquote: [0xc0, "`"], Minus: [0xbd, "-"], Equal: [0xbb, "="],
  BracketLeft: [0xdb, "["], BracketRight: [0xdd, "]"], Semicolon: [0xba, ";"], Quote: [0xde, "'"],
  Comma: [0xbc, ","], Period: [0xbe, "."], Slash: [0xbf, "/"], Backslash: [0xdc, "\\"],
  ArrowUp: [0x26, "↑"], ArrowDown: [0x28, "↓"], ArrowLeft: [0x25, "←"], ArrowRight: [0x27, "→"],
  Insert: [0x2d, "Insert"], Delete: [0x2e, "Delete"], Home: [0x24, "Home"], End: [0x23, "End"],
  PageUp: [0x21, "Page Up"], PageDown: [0x22, "Page Down"],
};

/** Tastă din tastatură → [cod virtual Windows, etichetă]. */
export function keyToVk(e: KeyboardEvent): [number, string] | null {
  const c = e.code;
  if (/^Key[A-Z]$/.test(c)) return [c.charCodeAt(3), c.slice(3)];
  if (/^Digit[0-9]$/.test(c)) return [c.charCodeAt(5), c.slice(5)];
  if (/^Numpad[0-9]$/.test(c)) return [0x60 + Number(c.slice(6)), `Num ${c.slice(6)}`];
  const f = /^F([0-9]{1,2})$/.exec(c);
  if (f) return [0x6f + Number(f[1]), c];
  return VK_SPECIAL[c] ?? null;
}

/** Butoanele mouse-ului → cod virtual. */
export function mouseToVk(button: number): [number, string] | null {
  switch (button) {
    case 0: return [0x01, "Click stânga"];
    case 1: return [0x04, "Click rotiță"];
    case 2: return [0x02, "Click dreapta"];
    case 3: return [0x05, "Mouse 4"];
    case 4: return [0x06, "Mouse 5"];
    default: return null;
  }
}

/** Combinație de taste → șir de scurtătură Tauri (ex. „Ctrl+Shift+F9”). */
export function keyToAccelerator(e: KeyboardEvent): string | null {
  const mods = ["Control", "Shift", "Alt", "Meta"];
  if (mods.some((m) => e.key === m)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  let key = e.code;
  if (/^Key[A-Z]$/.test(key)) key = key.slice(3);
  else if (/^Digit[0-9]$/.test(key)) key = key.slice(5);
  else if (!/^(F[0-9]{1,2}|Numpad[0-9]|Space|Insert|Delete|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right))$/.test(key)) return null;
  // Tastele simple (litere/cifre) fără modificator ar bloca scrisul în joc.
  if (parts.length === 0 && !/^(F[0-9]{1,2}|Numpad[0-9]|Insert|Home|End|PageUp|PageDown)$/.test(key)) return null;
  parts.push(key);
  return parts.join("+");
}
