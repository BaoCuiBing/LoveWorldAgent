<template>
    <Dialog :open="visible">
        <DialogContent class="w-[320px] max-w-[calc(100vw-32px)] [&>button]:hidden" @escape-key-down.prevent @pointer-down-outside.prevent>
            <DialogHeader>
                <DialogTitle class="sr-only">退出中</DialogTitle>
                <DialogDescription class="sr-only">正在停止终端、MCP 与内置工具服务进程</DialogDescription>
            </DialogHeader>
            <div class="qs-body">
                <LoaderCircle class="qs-spin animate-spin" :size="20" />
                <span class="qs-text">退出中</span>
            </div>
        </DialogContent>
    </Dialog>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { LoaderCircle } from 'lucide-vue-next'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { exitApp, quitApp } from '../../api/request_data'
const visible = ref(false)
let running = false
const onQuitRequested = async () => {
    if (running) return
    running = true
    visible.value = true
    try {
        await quitApp()
    } catch { /* 清理超时或失败仍继续退出 */ }
    visible.value = false
    try {
        await exitApp()
    } catch { /* 进程即将结束 */ }
}
onMounted(() => window.addEventListener('lwa:quit-requested', onQuitRequested))
onBeforeUnmount(() => window.removeEventListener('lwa:quit-requested', onQuitRequested))
</script>

<style scoped>
.qs-body { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 14px 8px; color: var(--foreground); }
.qs-spin { color: var(--primary); flex: none; }
.qs-text { font-size: 14px; font-weight: 600; }
</style>
