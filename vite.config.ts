import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue()],

  // 多页入口：island 是原子岛的独立轻量页面（无 Vue/naive-ui/echarts，毫秒级加载）
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        island: "island.html",
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
      // 4. Windows 上原生文件事件会间歇性漏掉（表现为"改了没生效、HMR 不发"），
      //    改轮询后可靠；代价是很小的 CPU 占用
      usePolling: true,
      interval: 300,
    },
  },
}));
