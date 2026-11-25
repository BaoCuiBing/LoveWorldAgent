<template>
    <div class="be-root">
        <div class="be-section">
            <div class="be-title">内置浏览器</div>
            <div class="panel-card be-card">
                <div class="be-row">
                    <div class="be-row-main">
                        <div class="be-row-label">启用内置浏览器</div>
                        <div class="be-row-desc">关闭后不再注册浏览器工具；已打开的浏览器窗口不会被强制结束，仅对后续会话生效</div>
                    </div>
                    <Switch v-model="form.enabled" @update:model-value="save" />
                </div>
                <div class="be-row">
                    <div class="be-row-main">
                        <div class="be-row-label">自动截图</div>
                        <div class="be-row-desc">浏览器操作后自动截取屏幕截图，仅用于展示，不消耗 Token</div>
                    </div>
                    <Switch v-model="form.auto_screenshot" @update:model-value="save" />
                </div>
            </div>
        </div>
        <div class="be-section">
            <div class="panel-card be-card">
                <div class="be-row">
                    <div class="be-row-main">
                        <div class="be-row-label">从本机浏览器导入</div>
                        <div class="be-row-desc">从已安装的 Chrome / Edge 解密并导入 Cookie 到内置浏览器</div>
                    </div>
                    <Button size="sm" variant="outline" :disabled="importing" @click="openImport">
                        <LoaderCircle v-if="importing" class="animate-spin" :size="14" />
                        <span>导入</span>
                    </Button>
                </div>
                <div class="be-row">
                    <div class="be-row-main">
                        <div class="be-row-label">截图文件</div>
                        <div class="be-row-desc">清空浏览器操作产生的本地截图</div>
                    </div>
                    <Button size="sm" variant="outline" class="be-danger" :disabled="clearing" @click="clearShots">
                        <LoaderCircle v-if="clearing" class="animate-spin" :size="14" />
                        <span>清除截图文件</span>
                    </Button>
                </div>
            </div>
        </div>
        <Dialog v-model:open="importVisible">
            <DialogContent class="w-[420px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>从浏览器导入 Cookie</DialogTitle>
                    <DialogDescription class="sr-only">选择本机浏览器来源并导入 Cookie</DialogDescription>
                </DialogHeader>
                <div class="be-import">
                    <div class="be-import-row">
                        <span class="be-import-label">来源</span>
                        <Select v-model="importSource">
                            <SelectTrigger class="be-import-select"><SelectValue placeholder="选择浏览器" /></SelectTrigger>
                            <SelectContent>
                                <SelectItem v-for="s in sources" :key="s.key" :value="s.key" :disabled="!s.available">{{ `${s.name}${s.available ? '' : '（未安装）'}` }}</SelectItem>
                            </SelectContent>
                        </Select>
                    </div>
                    <div class="be-import-row">
                        <div class="be-row-main">
                            <div class="be-row-label">包含已过期 Cookie</div>
                            <div class="be-row-desc">默认只导入未过期的 Cookie</div>
                        </div>
                        <Switch v-model="includeExpired" />
                    </div>
                </div>
                <DialogFooter>
                    <Button size="sm" variant="outline" @click="importVisible = false">取消</Button>
                    <Button size="sm" :disabled="!importSource || importing" @click="doImport">开始导入</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { LoaderCircle } from 'lucide-vue-next'
import { Button } from '../../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../../components/shadcn_ui/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../components/shadcn_ui/ui/select'
import { Switch } from '../../../components/shadcn_ui/ui/switch'
import { toast } from '../../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../../components/shadcn_ui/lib/useConfirm'
import { clearScreenshots, getBrowserSettings, getImportSources, importBrowserCookies, saveBrowserSettings } from '../../../api/request_data'
const emit = defineEmits(['open-browser'])
const form = reactive({ auto_screenshot: true, enabled: true })
const importing = ref(false)
const clearing = ref(false)
const importVisible = ref(false)
const importSource = ref('')
const includeExpired = ref(false)
const sources = ref([])
const load = async () => {
    try {
        const data = await getBrowserSettings()
        Object.keys(form).forEach((key) => {
            if (data[key] !== undefined) form[key] = data[key]
        })
    } catch (err) {
        toast.error(`读取浏览器设置失败：${err.message}`)
    }
}
const save = async () => {
    try {
        const saved = await saveBrowserSettings({ ...form })
        Object.keys(form).forEach((key) => {
            if (saved[key] !== undefined) form[key] = saved[key]
        })
        toast.success('浏览器设置已保存')
        emit('open-browser', { action: 'settings-saved' })
    } catch (err) {
        toast.error(`保存浏览器设置失败：${err.message}`)
    }
}
const openImport = async () => {
    try {
        sources.value = await getImportSources()
        importSource.value = sources.value.find((s) => s.available)?.key || ''
        importVisible.value = true
    } catch (err) {
        toast.error(`读取浏览器来源失败：${err.message}`)
    }
}
const reasonText = (reasons = {}) => {
    const labels = { v20: '新版 App-Bound 加密，暂不支持', empty: '空值', decrypt: '解密失败', expired: '已过期', locked: '浏览器占用', read: '读取失败' }
    return Object.keys(labels).filter((key) => (reasons[key] || 0) > 0).map((key) => `${reasons[key]} 条${labels[key]}`).join('，')
}
const doImport = async () => {
    if (!importSource.value) return
    importing.value = true
    try {
        const res = await importBrowserCookies(importSource.value, includeExpired.value)
        const total = res.total || 0
        const imported = res.imported || 0
        const reasons = res.reasons || {}
        const errors = res.errors || []
        const text = reasonText(reasons)
        if (errors.some((item) => item.reason === 'locked')) {
            toast.error('读取失败：Chrome / Edge 正在运行，Cookie 数据库被占用，请完全退出浏览器后重试')
        } else if (total === 0) {
            const onlyV20 = (reasons.v20 || 0) > 0 && Object.keys(reasons).every((key) => key === 'v20' || !(reasons[key] > 0))  // 仅 App-Bound 加密时给出针对性提示
            toast.warning(onlyV20 ? '本机浏览器 Cookie 均为新版 App-Bound 加密，暂不支持' : `未读取到可用 Cookie${text ? `：${text}` : ''}`)
        } else toast.success(`已从 ${importSource.value} 导入 ${imported}/${total} 条 Cookie${text ? `（跳过：${text}）` : ''}`)
        importVisible.value = false
    } catch (err) {
        toast.error(`导入失败：${err.message}`)
    } finally {
        importing.value = false
    }
}
const clearShots = async () => {
    const ok = await confirm({ title: '清除截图', description: '将删除浏览器产生的全部本地截图文件，是否继续？', confirmText: '清除', destructive: true })
    if (!ok) return
    clearing.value = true
    try {
        const res = await clearScreenshots()
        if (res.ok) toast.success(`已清除 ${res.data.deleted} 张截图`)
        else toast.error('清除截图失败')
    } catch (err) {
        toast.error(`清除截图失败：${err.message}`)
    } finally {
        clearing.value = false
    }
}
onMounted(load)
</script>

<style scoped>
.be-root { display: flex; flex-direction: column; gap: 18px; }
.be-section { display: flex; flex-direction: column; gap: 8px; }
.be-title { font-size: 14px; font-weight: 600; }
.be-card { display: flex; flex-direction: column; overflow: hidden; }
.be-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 14px 16px; }
.be-row + .be-row { border-top: 1px solid var(--border); }
.be-row-main { flex: 1; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.be-row-label { font-size: 13px; font-weight: 600; }
.be-row-desc { font-size: 12px; color: var(--muted-foreground); }
.be-danger { color: var(--destructive); border-color: var(--destructive); }
.be-import { display: flex; flex-direction: column; gap: 16px; }
.be-import-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.be-import-label { flex: none; font-size: 13px; font-weight: 600; }
.be-import-select { flex: 1; min-width: 0; }
</style>
