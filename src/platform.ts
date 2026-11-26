// Platform flag for the few places the UI differs between Windows and Linux.
// WebView2 reports "Windows" in its user agent, WebKitGTK reports "Linux".
export const isLinux = /Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent);

if (isLinux) {
  document.documentElement.classList.add('platform-linux');
}
