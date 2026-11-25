import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
    // 桌面端以 file:// 加载构建产物，资源必须使用相对路径
    base: './',
    plugins: [vue(), tailwindcss()],
    optimizeDeps: {
        // 避免预构建改写 monaco worker 的 import.meta.url
        exclude: ['monaco-editor'],
    },
    resolve: {
        alias: {
            '@': fileURLToPath(new URL('./src', import.meta.url)),
        },
    },
    worker: {
        format: 'es',
    },
    build: {
        // 直接输出到项目根的 webdist，pywebview 从该目录加载
        outDir: '../webdist',
        emptyOutDir: true,
    },
    server: {
        port: 5173,
        // 固定 IPv4，避免 Edge 解析 localhost 到 ::1 时连接被拒
        host: '127.0.0.1',
        strictPort: true,
        proxy: {
            // 前端 /api 请求转发到本机 HTTP 服务（backend/app.py）；ws:true 让 /api/terminal/ws 长连接也走同一代理
            '/api': {
                target: 'http://127.0.0.1:8866',
                changeOrigin: true,
                ws: true,
            },
        },
    },
})
