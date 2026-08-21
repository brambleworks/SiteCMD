(function () {
  window.__SHK_CWV__ = {
    lcp_ms: null,
    cls: null,
    fcp_ms: null,
    ttfb_ms: null,
    observed_long_task_blocking_ms: null,
    js_errors: [],
    js_error_count: 0,
  };
  var clsValue = 0;
  var clsSessionValue = 0;
  var clsSessionEntries = [];
  var longTaskEntries = [];

  function updateObservedLongTaskBlocking() {
    var fcp = window.__SHK_CWV__.fcp_ms;
    if (typeof fcp !== "number") {
      return;
    }
    var blockingValue = 0;
    for (var task of longTaskEntries) {
      // Count blocking only after the task's 50 ms grace period and FCP.
      var taskEnd = task.startTime + task.duration;
      var blockingStart = Math.max(task.startTime + 50, fcp);
      var blocking = taskEnd - blockingStart;
      if (blocking > 0) {
        blockingValue += blocking;
      }
    }
    window.__SHK_CWV__.observed_long_task_blocking_ms = blockingValue;
  }

  try {
    new PerformanceObserver(function (list) {
      var entries = list.getEntries();
      if (entries.length > 0) {
        window.__SHK_CWV__.lcp_ms = entries[entries.length - 1].startTime;
      }
    }).observe({ type: "largest-contentful-paint", buffered: true });
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  try {
    new PerformanceObserver(function (list) {
      for (var entry of list.getEntries()) {
        if (entry.hadRecentInput) {
          continue;
        }

        var firstEntry = clsSessionEntries[0];
        var previousEntry = clsSessionEntries[clsSessionEntries.length - 1];
        if (
          clsSessionEntries.length > 0 &&
          entry.startTime - previousEntry.startTime < 1000 &&
          entry.startTime - firstEntry.startTime < 5000
        ) {
          clsSessionValue += entry.value;
          clsSessionEntries.push(entry);
        } else {
          clsSessionValue = entry.value;
          clsSessionEntries = [entry];
        }

        if (clsSessionValue > clsValue) {
          clsValue = clsSessionValue;
          window.__SHK_CWV__.cls = clsValue;
        }
      }
    }).observe({ type: "layout-shift", buffered: true });
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  try {
    new PerformanceObserver(function (list) {
      for (var entry of list.getEntries()) {
        if (entry.name === "first-contentful-paint") {
          window.__SHK_CWV__.fcp_ms = entry.startTime;
          updateObservedLongTaskBlocking();
        }
      }
    }).observe({ type: "paint", buffered: true });
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  // Unlike Lighthouse TBT, this Chromium-only metric ends when SiteCMD reads the page.
  try {
    var supportedTypes = PerformanceObserver.supportedEntryTypes || [];
    if (supportedTypes.indexOf("longtask") !== -1) {
      new PerformanceObserver(function (list) {
        for (var entry of list.getEntries()) {
          longTaskEntries.push(entry);
        }
        updateObservedLongTaskBlocking();
      }).observe({ type: "longtask", buffered: true });
    }
  } catch (e) {
    // Unsupported performance entry types leave this optional metric unset.
  }

  // Initialization-time listeners count all load errors but store at most ten messages.
  function shkRecordError(message) {
    try {
      var c = window.__SHK_CWV__;
      c.js_error_count += 1;
      if (c.js_errors.length < 10) {
        c.js_errors.push(String(message).slice(0, 200));
      }
    } catch (e) {
      // Error collection must never interfere with the page being measured.
    }
  }
  try {
    window.addEventListener("error", function (e) {
      var msg = (e && e.message) || "Script error";
      if (e && e.filename) {
        msg += " (" + e.filename + (e.lineno ? ":" + e.lineno : "") + ")";
      }
      shkRecordError(msg);
    });
    window.addEventListener("unhandledrejection", function (e) {
      var reason = e && e.reason;
      var msg = (reason && (reason.message || reason.toString())) || "unknown reason";
      shkRecordError("Unhandled promise rejection: " + msg);
    });
  } catch (e) {
    // A page that blocks listener installation still returns the other metrics.
  }

  try {
    var navEntries = performance.getEntriesByType("navigation");
    if (navEntries.length > 0) {
      window.__SHK_CWV__.ttfb_ms = navEntries[0].responseStart;
    }
  } catch (e) {
    // Unsupported navigation timing leaves TTFB unset for the fallback below.
  }

  if (!window.__SHK_CWV__.ttfb_ms && performance.timing) {
    try {
      var t = performance.timing;
      if (t.responseStart > 0 && t.navigationStart > 0) {
        window.__SHK_CWV__.ttfb_ms = t.responseStart - t.navigationStart;
      }
    } catch (e) {
      // Legacy navigation timing is optional and can be unavailable by policy.
    }
  }
})();
