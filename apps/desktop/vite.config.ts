import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { execFileSync } from "node:child_process";
import fs from "fs";
import path from "path";

const host = process.env.TAURI_DEV_HOST ?? "localhost";
const configDir = import.meta.dirname;
const desktopScriptTargets = ["chrome105", "safari13.1"];
const desktopStyleTargets = ["chrome111", "safari16.2"];

// Read the shipped version from the canonical release-managed file.
const appVersion: string = JSON.parse(
  fs.readFileSync(path.resolve(configDir, "package.json"), "utf8"),
).version;

function resolveSourceCommit(): string {
  const configured = process.env.SITECMD_SOURCE_COMMIT?.trim() ?? "";
  if (/^[0-9a-f]{40}$/i.test(configured)) return configured.toLowerCase();

  try {
    const commit = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: path.resolve(configDir, "../.."),
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
    return /^[0-9a-f]{40}$/i.test(commit) ? commit.toLowerCase() : "";
  } catch {
    return "";
  }
}

const sourceCommit = resolveSourceCommit();

function getNodeModulePackageName(id: string): string | null {
  const marker = "node_modules/";
  const index = id.lastIndexOf(marker);
  if (index === -1) return null;
  const modulePath = id.slice(index + marker.length);
  const parts = modulePath.split("/");
  if (parts[0]?.startsWith("@")) {
    return parts.length >= 2 ? `${parts[0]}/${parts[1]}` : null;
  }
  return parts[0] ?? null;
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  define: {
    "import.meta.env.VITE_APP_VERSION": JSON.stringify(appVersion),
    "import.meta.env.VITE_SOURCE_COMMIT": JSON.stringify(sourceCommit),
  },
  resolve: {
    alias: {
      "@": path.resolve(configDir, "./src"),
    },
  },
  build: {
    target: desktopScriptTargets,
    cssTarget: desktopStyleTargets,
    cssMinify: "lightningcss",
    // Allow the known lazy PDF bundle while still warning on unexpected growth.
    chunkSizeWarningLimit: 1600,
    rollupOptions: {
      output: {
        manualChunks(id) {
          const pkg = getNodeModulePackageName(id);
          if (!pkg) return;

          if (pkg === "react-dom" || pkg === "react") return "vendor-react";
          if (pkg.startsWith("@tauri-apps/")) return "vendor-tauri";
          if (pkg === "lucide-react") return "vendor-icons";

          if (pkg === "@react-pdf/pdfkit" || pkg === "fontkit") {
            return "vendor-react-pdf-pdfkit";
          }

          if (
            pkg === "@react-pdf/layout" ||
            pkg === "@react-pdf/textkit" ||
            pkg === "@react-pdf/font" ||
            pkg === "yoga-layout"
          ) {
            return "vendor-react-pdf-layout";
          }

          if (
            pkg === "@react-pdf/renderer" ||
            pkg === "@react-pdf/render" ||
            pkg === "@react-pdf/reconciler" ||
            pkg === "@react-pdf/primitives" ||
            pkg === "@react-pdf/fns" ||
            pkg === "@react-pdf/image" ||
            pkg === "@react-pdf/stylesheet" ||
            pkg === "@react-pdf/png-js" ||
            pkg === "@react-pdf/types" ||
            pkg === "queue"
          ) {
            return "vendor-react-pdf-core";
          }
        },
      },
    },
  },
  // Prevent vite from obscuring rust errors
  clearScreen: false,
  server: {
    // Tauri expects a fixed port
    port: 5173,
    strictPort: true,
    host,
    hmr: {
      protocol: "ws",
      host,
      port: 5174,
    },
    watch: {
      // Tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
});
