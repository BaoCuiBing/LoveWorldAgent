<template>
    <div class="cu-root">
        <div class="cu-section">
            <div class="cu-title">电脑控制</div>
            <div class="panel-card cu-card">
                <div class="cu-row">
                    <div class="cu-row-main">
                        <div class="cu-row-label">启用</div>
                        <div class="cu-row-desc">开启后智能体可使用电脑控制工具；关闭后直接拒绝执行并结束后台服务</div>
                    </div>
                    <Switch v-model="form.enabled" :disabled="loading" @update:model-value="save('enabled', $event)" />
                </div>
                <div class="cu-row">
                    <div class="cu-row-main">
                        <div class="cu-row-label">自动截图</div>
                        <div class="cu-row-desc">桌面操作后自动截取屏幕截图，仅用于右侧面板展示，不消耗 Token</div>
                    </div>
                    <Switch v-model="form.auto_screenshot" :disabled="loading" @update:model-value="save('auto_screenshot', $event)" />
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { Switch } from '../../../components/shadcn_ui/ui/switch'
import { toast } from '../../../components/shadcn_ui/lib/useToast'
import { getComputerUse, saveComputerUse } from '../../../api/request_data'
const form = reactive({ enabled: true, auto_screenshot: true })
const loading = ref(false)
const apply = (data) => {
    if (!data) return
    if (data.enabled !== undefined) form.enabled = Boolean(data.enabled)
    if (data.auto_screenshot !== undefined) form.auto_screenshot = Boolean(data.auto_screenshot)
}
const load = async () => {
    loading.value = true
    try {
        apply(await getComputerUse())
    } catch (err) {
        toast.error(`读取电脑控制设置失败：${err.message}`)
    } finally {
        loading.value = false
    }
}
const save = async (key, value) => {
    const prev = !Boolean(value)
    loading.value = true
    try {
        const data = await saveComputerUse({ [key]: Boolean(value) })
        apply(data)
        if (key === 'enabled') toast.success(form.enabled ? '电脑控制已开启' : '电脑控制已关闭')
        else toast.success(form.auto_screenshot ? '自动截图已开启' : '自动截图已关闭')
    } catch (err) {
        form[key] = prev
        toast.error(`保存电脑控制设置失败：${err.message}`)
    } finally {
        loading.value = false
    }
}
onMounted(load)
</script>

<style scoped>
.cu-root { display: flex; flex-direction: column; gap: 16px; }
.cu-section { display: flex; flex-direction: column; gap: 8px; }
.cu-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.cu-card { padding: 4px 12px; }
.cu-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 0; }
.cu-row + .cu-row { border-top: 1px solid var(--border); }
.cu-row-main { flex: 1; display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.cu-row-label { font-size: 13px; color: var(--foreground); }
.cu-row-desc { font-size: 12px; color: var(--muted-foreground); }
</style>
