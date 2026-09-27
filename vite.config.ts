import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri 要求：dev 端口固定、不自动清屏（否则看不到 Rust 报错），
// 并且监听时忽略 src-tauri（避免前端 watch 触发 Rust 重编的死循环）。
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: '0.0.0.0',
    allowedHosts: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    target: 'es2021',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
  },
})
