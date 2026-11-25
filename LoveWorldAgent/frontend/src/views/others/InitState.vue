<template>
    <div class="fixed inset-0 z-[9999] flex select-none items-center justify-center transition-opacity duration-300" :class="leaving ? 'opacity-0 pointer-events-none' : 'opacity-100'" style="background: var(--background, #181818); color: var(--foreground, #f0f0f0);">
        <div class="flex flex-col items-center gap-3.5">
            <LoaderCircle class="animate-spin text-[var(--primary,#409eff)]" :size="30" :stroke-width="2.2" />
            <span class="text-[13px] font-medium tracking-[0.08em] text-[var(--foreground,#f0f0f0)]">内核启动中</span>
        </div>
    </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { LoaderCircle } from 'lucide-vue-next'
import { checkHealth } from '../../api/request_data'
const POLL_MS = 250 // 内核健康检查轮询间隔
const READY_TIMEOUT_MS = 10000 // 主界面就绪兜底超时
const FADE_MS = 300 // 淡出动画时长
const emit = defineEmits(['kernel-ready', 'done'])
const leaving = ref(false)
let disposed = false
let checking = false
let pollTimer = null
let fallbackTimer = null
let fadeTimer = null
const finishInit = () => {
    if (disposed || leaving.value) return
    leaving.value = true
    window.removeEventListener('lwa:app-ready', finishInit)
    if (fallbackTimer) { clearTimeout(fallbackTimer); fallbackTimer = null }
    fadeTimer = setTimeout(() => { if (!disposed) emit('done') }, FADE_MS)
}
const pollKernel = async () => {
    if (disposed || checking) return
    checking = true
    const ok = await checkHealth()
    checking = false
    if (disposed) return
    if (ok) {
        window.addEventListener('lwa:app-ready', finishInit, { once: true })
        emit('kernel-ready')
        fallbackTimer = setTimeout(finishInit, READY_TIMEOUT_MS)
        return
    }
    pollTimer = setTimeout(pollKernel, POLL_MS)
}
onMounted(() => { pollKernel() })
onBeforeUnmount(() => {
    disposed = true
    window.removeEventListener('lwa:app-ready', finishInit)
    if (pollTimer) clearTimeout(pollTimer)
    if (fallbackTimer) clearTimeout(fallbackTimer)
    if (fadeTimer) clearTimeout(fadeTimer)
})
</script>
