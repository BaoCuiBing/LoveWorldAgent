<template>
    <Dialog v-model:open="visible">
        <DialogContent class="w-[440px] max-w-[calc(100vw-32px)]">
            <DialogHeader>
                <DialogTitle>清除数据</DialogTitle>
                <DialogDescription class="sr-only">确认清除所有会话与对话数据，此操作不可恢复</DialogDescription>
            </DialogHeader>
            <div class="cd-body">
                <div v-if="loading" class="cd-loading">正在统计数据...</div>
                <template v-else>
                    <div class="cd-stat">将删除 <strong>{{ preview.sessions }}</strong> 个会话、<strong>{{ preview.messages }}</strong> 条消息。</div>
                    <div class="cd-warn">此操作不可恢复。模型、权限、规则、技能等配置不会被清除。</div>
                    <div v-if="error" class="cd-error">{{ error }}</div>
                </template>
            </div>
            <DialogFooter>
                <Button size="sm" variant="outline" :disabled="clearing" @click="close">取消</Button>
                <Button size="sm" class="cd-confirm" :disabled="loading || clearing || running" @click="confirmClear">
                    <LoaderCircle v-if="clearing" class="animate-spin" :size="14" />
                    <span>{{ running ? '请先停止任务' : '确认清除' }}</span>
                </Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
</template>

<script setup>
import { reactive, ref, watch } from 'vue'
import { LoaderCircle } from 'lucide-vue-next'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { clearData, clearDataPreview } from '../../api/request_data'
const props = defineProps({ open: { type: Boolean, default: false } })
const emit = defineEmits(['update:open', 'cleared'])
const visible = ref(props.open)
const preview = reactive({ sessions: 0, messages: 0 })
const loading = ref(false)
const clearing = ref(false)
const error = ref('')
const running = ref(false)
watch(() => props.open, (value) => {
    visible.value = value
    if (value) loadPreview()
})
watch(visible, (value) => { if (!value) emit('update:open', false) })
const loadPreview = async () => {
    loading.value = true
    error.value = ''
    try {
        const data = await clearDataPreview()
        preview.sessions = data.sessions || 0
        preview.messages = data.messages || 0
        running.value = Boolean(data.running)
    } catch (err) {
        error.value = `统计数据失败：${err.message}`
    } finally {
        loading.value = false
    }
}
const close = () => { visible.value = false }
const confirmClear = async () => {
    if (running.value) return
    clearing.value = true
    error.value = ''
    try {
        await clearData()
        toast.success('对话数据已清除')
        visible.value = false
        emit('cleared')
    } catch (err) {
        error.value = err.message || '清除失败'
        toast.error(error.value)
    } finally {
        clearing.value = false
    }
}
</script>

<style scoped>
.cd-body { padding: 4px 0; font-size: 13px; color: var(--foreground); }
.cd-loading, .cd-warn { color: var(--muted-foreground); font-size: 12px; }
.cd-stat { margin-bottom: 8px; }
.cd-warn { line-height: 1.6; }
.cd-error { margin-top: 8px; color: #e5484d; font-size: 12px; }
.cd-confirm { background: #d92d20; color: #fff; }
.cd-confirm:hover { background: #b42318; }
.cd-confirm:disabled { opacity: 0.5; }
</style>
