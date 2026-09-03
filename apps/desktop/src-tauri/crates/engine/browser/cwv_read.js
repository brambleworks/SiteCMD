(function () {
  var c = window.__SHK_CWV__ || {
    lcp_ms: null,
    cls: null,
    fcp_ms: null,
    ttfb_ms: null,
    observed_long_task_blocking_ms: null,
    js_errors: [],
    js_error_count: 0,
  };
  window.__SHK_CWV__ = c;
  if (!c.js_errors) {
    c.js_errors = [];
  }
  if (typeof c.js_error_count !== "number") {
    c.js_error_count = c.js_errors.length;
  }

  try {
    if (!c.fcp_ms) {
      var paints = performance.getEntriesByType("paint") || [];
      for (var i = 0; i < paints.length; i++) {
        if (paints[i].name === "first-contentful-paint") {
          c.fcp_ms = paints[i].startTime;
        }
      }
    }
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  try {
    if (!c.lcp_ms) {
      var lcp = performance.getEntriesByType("largest-contentful-paint") || [];
      if (lcp.length > 0) {
        c.lcp_ms = lcp[lcp.length - 1].startTime;
      }
    }
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  try {
    if (c.cls === null || typeof c.cls === "undefined") {
      var supported = PerformanceObserver.supportedEntryTypes || [];
      if (supported.indexOf("layout-shift") !== -1) {
        var shifts = performance.getEntriesByType("layout-shift") || [];
        var cls = 0;
        var sessionValue = 0;
        var sessionEntries = [];
        for (var s = 0; s < shifts.length; s++) {
          var shift = shifts[s];
          if (shift.hadRecentInput) {
            continue;
          }
          var firstShift = sessionEntries[0];
          var previousShift = sessionEntries[sessionEntries.length - 1];
          if (
            sessionEntries.length > 0 &&
            shift.startTime - previousShift.startTime < 1000 &&
            shift.startTime - firstShift.startTime < 5000
          ) {
            sessionValue += shift.value;
            sessionEntries.push(shift);
          } else {
            sessionValue = shift.value;
            sessionEntries = [shift];
          }
          if (sessionValue > cls) {
            cls = sessionValue;
          }
        }
        c.cls = cls;
      }
    }
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  if (!c.ttfb_ms) {
    try {
      var nav = performance.getEntriesByType("navigation");
      if (nav.length > 0 && nav[0].responseStart > 0) {
        c.ttfb_ms = nav[0].responseStart;
      }
    } catch (e) {
      // Unsupported navigation timing leaves TTFB unset.
    }
  }

  try {
    if (
      c.observed_long_task_blocking_ms === null ||
      typeof c.observed_long_task_blocking_ms === "undefined"
    ) {
      var supportedLt = PerformanceObserver.supportedEntryTypes || [];
      if (supportedLt.indexOf("longtask") !== -1 && typeof c.fcp_ms === "number") {
        var tasks = performance.getEntriesByType("longtask") || [];
        var observedBlocking = 0;
        for (var lt = 0; lt < tasks.length; lt++) {
          var taskEnd = tasks[lt].startTime + tasks[lt].duration;
          var blockingStart = Math.max(tasks[lt].startTime + 50, c.fcp_ms);
          var blocking = taskEnd - blockingStart;
          if (blocking > 0) {
            observedBlocking += blocking;
          }
        }
        c.observed_long_task_blocking_ms = observedBlocking;
      }
    }
  } catch (e) {
    // Unsupported long-task timing leaves this optional metric unset.
  }

  // The adapter grades this sample only when it names the document it was
  // asked about.
  try {
    c.document_url = String(window.location.href);
  } catch (e) {
    c.document_url = null;
  }

  // Runtimes that can read an eval value take the return; the Tauri webview
  // reads window.__SHK_CWV__ through its chunked title bridge instead.
  return JSON.stringify(c);
})();
