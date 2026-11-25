<template>
    <div class="oo-root">
        <div class="oo-section">
            <div class="oo-title">{{ title }}</div>
            <div class="panel-card oo-card">
                <div class="oo-row">
                    <div class="oo-row-main">
                        <div class="oo-row-label">启用</div>
                        <div class="oo-row-desc">{{ enableDesc }}</div>
                    </div>
                    <Switch v-model="form.enabled" :disabled="loading" @update:model-value="saveSwitch('enabled', $event)" />
                </div>
                <div class="oo-row">
                    <div class="oo-row-main">
                        <div class="oo-row-label">自动截图</div>
                        <div class="oo-row-desc">执行{{ label }}操作后自动截取可见应用窗口，仅用于右侧面板展示，不消耗 Token</div>
                    </div>
                    <Switch v-model="form.autoScreenshot" :disabled="loading" @update:model-value="saveSwitch('autoScreenshot', $event)" />
                </div>
            </div>
        </div>
        <div class="oo-section">
            <div class="panel-card oo-card oo-fields">
                <div class="oo-field">
                    <div class="oo-row-label">备份保留数</div>
                    <Input v-model="backupKeep" type="number" min="1" max="30" />
                    <div class="oo-row-desc">保存前自动生成 .bak 备份，保留最近 N 份</div>
                </div>
                <div class="oo-actions">
                    <Button size="sm" :disabled="loading" @click="saveBackup">保存备份设置</Button>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { Switch } from '../../../components/shadcn_ui/ui/switch'
import { Input } from '../../../components/shadcn_ui/ui/input'
import { Button } from '../../../components/shadcn_ui/ui/button'
import { toast } from '../../../components/shadcn_ui/lib/useToast'
const props = defineProps({
    label: { type: String, required: true },
    title: { type: String, required: true },
    enableDesc: { type: String, required: true },
    fetchOption: { type: Function, required: true },
    saveOption: { type: Function, required: true },
})
const form = reactive({ enabled: true, autoScreenshot: true })
const backupKeep = ref(5)
const loading = ref(false)
const apply = (data) => {
    if (!data) return
    if (data.enabled !== undefined) form.enabled = Boolean(data.enabled)
    if (data.autoScreenshot !== undefined) form.autoScreenshot = Boolean(data.autoScreenshot)
    if (data.backupKeep !== undefined) backupKeep.value = Number(data.backupKeep) || 5
}
const load = async () => {
    loading.value = true
    try { apply(await props.fetchOption()) } catch (err) { toast.error(`读取 ${props.label} 设置失败：${err.message}`) } finally { loading.value = false }
}
const saveSwitch = async (key, value) => {
    const prev = !Boolean(value)
    loading.value = true
    try {
        apply(await props.saveOption({ [key]: Boolean(value) }))
        if (key === 'enabled') toast.success(form.enabled ? `${props.title}已开启` : `${props.title}已关闭`)
        else toast.success(form.autoScreenshot ? `${props.label}自动截图已开启` : `${props.label}自动截图已关闭`)
    } catch (err) {
        form[key] = prev
        toast.error(`保存 ${props.label} 设置失败：${err.message}`)
    } finally { loading.value = false }
}
const saveBackup = async () => {
    const keep = Math.min(30, Math.max(1, Number(backupKeep.value) || 5))
    loading.value = true
    try {
        apply(await props.saveOption({ backupKeep: keep }))
        toast.success(`${props.label}备份设置已保存`)
    } catch (err) {
        toast.error(`保存 ${props.label} 设置失败：${err.message}`)
        await load()
    } finally { loading.value = false }
}
onMounted(load)
</script>

<style scoped>
.oo-root { display: flex; flex-direction: column; gap: 16px; }
.oo-section { display: flex; flex-direction: column; gap: 8px; }
.oo-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.oo-card { padding: 4px 12px; }
.oo-fields { display: flex; flex-direction: column; gap: 12px; padding: 12px; }
.oo-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 0; }
.oo-row + .oo-row { border-top: 1px solid var(--border); }
.oo-row-main { flex: 1; display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.oo-field { display: flex; flex-direction: column; gap: 6px; }
.oo-row-label { font-size: 13px; color: var(--foreground); }
.oo-row-desc { font-size: 12px; color: var(--muted-foreground); }
.oo-actions { display: flex; justify-content: flex-end; }
</style>
