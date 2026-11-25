<template>
    <div class="system-edit">
        <div class="system-head">
            <span class="system-title">主题</span>
        </div>
        <div class="panel-card theme-card">
            <div class="theme-main">
                <div class="theme-name">外观模式</div>
                <div class="theme-desc">选择深色、浅色或跟随系统，切换后立即生效并保存到 config.json。</div>
            </div>
            <Tabs :model-value="theme" @update:model-value="onChange">
                <TabsList class="theme-tabs">
                    <TabsTrigger value="dark" :disabled="saving">深色</TabsTrigger>
                    <TabsTrigger value="light" :disabled="saving">浅色</TabsTrigger>
                    <TabsTrigger value="system" :disabled="saving">跟随系统</TabsTrigger>
                </TabsList>
            </Tabs>
        </div>
        <div class="system-head system-head-gap">
            <span class="system-title">窗口</span>
        </div>
        <div class="panel-card theme-card">
            <div class="theme-main">
                <div class="theme-name">记住窗口布局</div>
                <div class="theme-desc">默认开启：三栏宽度、右栏开关与右栏页签的每次调整都会写入 config.json，下次启动自动恢复。</div>
            </div>
            <Switch :model-value="remember" :disabled="layoutSaving" @update:model-value="onRememberChange" />
        </div>
    </div>
</template>

<script setup>
import { onMounted, ref } from 'vue'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '../../components/shadcn_ui/ui/tabs'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { getLayout, getTheme, saveLayout, saveTheme } from '../../api/request_data'
import { applyTheme, THEME_STORAGE_KEY } from '../../theme'
const theme = ref(localStorage.getItem(THEME_STORAGE_KEY) || 'dark')
const saving = ref(false)
const remember = ref(true)
const layoutSaving = ref(false)
let layout = { remember: true, sideWidth: null, rightWidth: null, rightOpen: false, rightTab: '' }
const onChange = async (value) => {
    if (!value || value === theme.value) return
    theme.value = value  // 与原分段按钮一致：先切换选中态，保存失败再回滚
    saving.value = true
    try {
        const data = await saveTheme(value)
        theme.value = data.theme
        applyTheme(data.theme)
        toast.success('主题已更新')
    } catch (err) {
        theme.value = localStorage.getItem(THEME_STORAGE_KEY) || 'dark'
        toast.error(`保存主题失败：${err.message}`)
    } finally {
        saving.value = false
    }
}
const onRememberChange = async (value) => {
    layoutSaving.value = true
    layout = { ...layout, remember: value }
    try {
        layout = await saveLayout({ remember: value })
        toast.success(value ? '已开启窗口布局记忆' : '已关闭窗口布局记忆')
        window.dispatchEvent(new CustomEvent('lwa:layout-remember', { detail: { remember: value } }))
    } catch (err) {
        remember.value = !value
        layout.remember = !value
        toast.error(`保存失败：${err.message}`)
    } finally {
        layoutSaving.value = false
    }
}
onMounted(async () => {
    try {
        const data = await getTheme()
        theme.value = data.theme || theme.value
    } catch {
        theme.value = localStorage.getItem(THEME_STORAGE_KEY) || 'dark'
    }
    try {
        const data = await getLayout()
        layout = { ...layout, ...data }
        remember.value = data.remember !== false
    } catch { /* 读取失败保持默认开启 */ }
})
</script>

<style scoped>
.system-edit { display: flex; flex-direction: column; gap: 12px; }
.system-head { display: flex; align-items: center; justify-content: space-between; }
.system-head-gap { margin-top: 4px; }
.system-title { font-size: 15px; font-weight: 600; }
.theme-card { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 14px 16px; }
.theme-main { display: flex; flex-direction: column; gap: 4px; }
.theme-name { font-size: 13px; font-weight: 600; }
.theme-desc { font-size: 12px; color: var(--muted-foreground); }
.theme-tabs { height: 28px; }
</style>
