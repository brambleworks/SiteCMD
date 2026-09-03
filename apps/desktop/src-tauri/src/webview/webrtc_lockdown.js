// Removes the interfaces a scanned page could use to open connections from
// the analyzer webview's network position without going through the resource
// loader. The private-network subresource rules sit on that loader, so they
// never see WebRTC ICE gathering or WebTransport sessions; taking the
// constructors away before any page script runs, in every frame, closes that
// path. Feature detection sees the interfaces as absent rather than broken,
// so pages that check before use keep running normally.
(() => {
  "use strict";
  const names = ["RTCPeerConnection", "webkitRTCPeerConnection", "WebTransport"];
  for (const name of names) {
    let owner = globalThis;
    while (owner && !Object.prototype.hasOwnProperty.call(owner, name)) {
      owner = Object.getPrototypeOf(owner);
    }
    if (!owner) continue;
    try {
      delete owner[name];
    } catch (_notConfigurable) {
      try {
        Object.defineProperty(owner, name, {
          value: undefined,
          writable: false,
          configurable: false,
          enumerable: false,
        });
      } catch (_alreadyLocked) {
        // A frozen global already cannot hand the constructor back.
      }
    }
  }
})();
