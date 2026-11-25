/**
 * True when "/" should focus the place-search box: not while typing in a
 * field, not with a modifier, and not while a drawer/sheet/dialog is open.
 */
export function shouldFocusSearch({ key, inField, modifier, blocked }) {
  return key === "/" && !inField && !modifier && !blocked;
}
