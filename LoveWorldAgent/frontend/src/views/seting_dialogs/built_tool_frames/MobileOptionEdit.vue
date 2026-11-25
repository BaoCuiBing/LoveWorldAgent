<template>
    <div class="mo-root">
        <div class="mo-section">
            <div class="mo-title">移动端操作</div>
            <div class="panel-card mo-card">
                <div class="mo-row">
                    <div class="mo-row-main">
                        <div class="mo-row-label">启用</div>
                        <div class="mo-row-desc">开启后智能体可使用 28 个 mobile_* 工具；关闭后直接拒绝执行并结束后台服务</div>
                    </div>
                    <Switch v-model="enabled" :disabled="loading" @update:model-value="saveEnabled" />
                </div>
            </div>
        </div>
        <div class="mo-section">
            <div class="panel-card mo-card mo-fields">
                <div class="mo-field">
                    <div class="mo-row-label">adb 路径</div>
                    <Input v-model="adbPath" placeholder="留空则自动探测 ANDROID_HOME / PATH / 随包 platform-tools" />
                    <div class="mo-row-desc">可填 adb.exe 完整路径，或 platform-tools 目录</div>
                </div>
                <div class="mo-field">
                    <div class="mo-row-label">默认设备</div>
                    <Input v-model="defaultDevice" placeholder="留空时仅一台设备自动选择，多台需填写序列号" />
                    <div class="mo-row-desc">设备序列号来自 adb devices；多设备未填写时工具返回 DEVICE_REQUIRED</div>
                </div>
                <div class="mo-field">
                    <div class="mo-row-label">截图最大边</div>
                    <Input v-model="screenshotMaxSize" type="number" min="320" max="4096" />
                    <div class="mo-row-desc">截图返回给模型时的最大边长，默认 1280</div>
                </div>
                <div class="mo-actions">
                    <Button size="sm" :disabled="loading" @click="saveConfig">保存移动端设置</Button>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { onMounted, ref } from 'vue'
import { Switch } from '../../../components/shadcn_ui/ui/switch'
import { Input } from '../../../components/shadcn_ui/ui/input'
import { Button } from '../../../components/shadcn_ui/ui/button'
import { toast } from '../../../components/shadcn_ui/lib/useToast'
import { getMobileOption, saveMobileOption } from '../../../api/request_data'
const enabled = ref(true)
const adbPath = ref('')
const defaultDevice = ref('')
const screenshotMaxSize = ref(1280)
const loading = ref(false)
const currentPayload = () => ({
    enabled: enabled.value,
    adbPath: adbPath.value.trim(),
    defaultDevice: defaultDevice.value.trim(),
    screenshotMaxSize: Number(screenshotMaxSize.value) || 1280,
})
const apply = (data) => {
    enabled.value = Boolean(data && data.enabled)
    adbPath.value = String((data && data.adbPath) || '')
    defaultDevice.value = String((data && data.defaultDevice) || '')
    screenshotMaxSize.value = Number(data && data.screenshotMaxSize) || 1280
}
const load = async () => {
    loading.value = true
    try { apply(await getMobileOption()) } catch (err) { toast.error(`读取移动端设置失败：${err.message}`) } finally { loading.value = false }
}
const persist = async (payload, message) => {
    loading.value = true
    try { apply(await saveMobileOption(payload)); if (message) toast.success(message) } catch (err) { toast.error(`保存移动端设置失败：${err.message}`); await load() } finally { loading.value = false }
}
const saveEnabled = () => persist(currentPayload(), enabled.value ? '移动端操作已开启' : '移动端操作已关闭')
const saveConfig = () => persist(currentPayload(), '移动端设置已保存')
onMounted(load)
</script>

<style scoped>
.mo-root { display: flex; flex-direction: column; gap: 16px; }
.mo-section { display: flex; flex-direction: column; gap: 8px; }
.mo-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.mo-card { padding: 4px 12px; }
.mo-fields { display: flex; flex-direction: column; gap: 12px; padding: 12px; }
.mo-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 0; }
.mo-row-main { display: flex; flex-direction: column; gap: 3px; }
.mo-field { display: flex; flex-direction: column; gap: 6px; }
.mo-row-label { font-size: 13px; color: var(--foreground); }
.mo-row-desc { font-size: 12px; color: var(--muted-foreground); }
.mo-actions { display: flex; justify-content: flex-end; }
</style>