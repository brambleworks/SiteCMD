// Answers one chunk request from the analyzer's title bridge. The Rust side
// (title_bridge.rs) wraps this function with its arguments and evaluates it
// once per chunk; the frame grammar and the reason the payload is chunked
// are documented there.
(function (globalName, marker, index, chunkChars) {
  var value = window[globalName];
  if (value === undefined || value === null) {
    document.title = marker + "pending";
    return;
  }
  // Encode once: the Web Vitals observer keeps mutating its object while
  // chunks are served, and every chunk has to come from the same snapshot.
  var cache = window.__SHK_TITLE_BRIDGE__ || (window.__SHK_TITLE_BRIDGE__ = {});
  var encoded = cache[globalName];
  if (typeof encoded !== "string") {
    var bytes = new TextEncoder().encode(JSON.stringify(value));
    var binary = "";
    for (var offset = 0; offset < bytes.length; offset += 0x8000) {
      binary += String.fromCharCode.apply(null, bytes.subarray(offset, offset + 0x8000));
    }
    encoded = btoa(binary);
    cache[globalName] = encoded;
  }
  var total = Math.max(1, Math.ceil(encoded.length / chunkChars));
  var start = index * chunkChars;
  document.title = marker + index + "/" + total + ":" + encoded.slice(start, start + chunkChars);
});
