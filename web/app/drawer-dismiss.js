/**
 * Pure hit test for dismissing the web drawer on an outside click.
 * Only a click on the visible map closes it; the drawer, its toggle and the
 * rest of the chrome are excluded.
 */
export function drawerOutsideClickDismisses({ open, inDrawer, onToggle, inMap }) {
  return Boolean(open && inMap && !inDrawer && !onToggle);
}
