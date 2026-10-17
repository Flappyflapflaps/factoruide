/** Put a text in the clipboard: a selected text copied at once (the clipboard API waits for the window's focus in WebView2), else the clipboard API. */
export function copyText(text: string): Promise<void> {
  const area = document.createElement("textarea");
  area.value = text;
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  return ok ? Promise.resolve() : navigator.clipboard.writeText(text);
}
