// Callers retain ownership of focus and playback when a dialog is dismissed.
export function bindDialogDismiss(dialog: HTMLDialogElement, dismiss: () => void) {
  const outside = (event: MouseEvent) => {
    const box = dialog.getBoundingClientRect();
    return event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom;
  };
  let pressedOutside = false;
  dialog.addEventListener('pointerdown', (event) => { pressedOutside = event.target === dialog && outside(event); });
  dialog.addEventListener('click', (event) => {
    if (pressedOutside && event.target === dialog && outside(event)) dismiss();
    pressedOutside = false;
  });
  dialog.addEventListener('cancel', (event) => { event.preventDefault(); dismiss(); });
}
