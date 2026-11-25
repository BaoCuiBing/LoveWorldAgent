<template>
    <div class="model-edit">
        <header class="model-head">
            <span class="model-title">模型管理</span>
            <Button variant="ghost" size="icon-sm" aria-label="刷新模型列表" :disabled="loading" @click="loadList"><RefreshCw :size="18" :class="{ 'animate-spin': loading }" /></Button>
        </header>
        <div class="model-card panel-card">
            <div class="model-top">
                <div class="model-desc">
                    <div class="model-desc-text">配置 API key 添加更多可用模型，预置模型默认使用稳定版本。</div>
                </div>
                <Button @click="onCreate"><Plus :size="14" /><span>添加模型</span></Button>
            </div>
            <div class="model-list">
                <div v-if="loadError" class="model-error">{{ loadError }}</div>
                <div v-else-if="!loading && !items.length" class="empty-state">
                    <Sparkles :size="36" />
                    <div class="empty-title">暂无模型</div>
                    <div class="empty-hint">点击添加模型以接入你的第一个服务商</div>
                </div>
                <table v-else class="model-table">
                    <thead>
                        <tr><th>模型</th><th>服务商</th><th>多模态</th><th class="col-op">操作</th></tr>
                    </thead>
                    <tbody>
                        <tr v-for="(item, index) in items" :key="`${item.provider}-${item.model}-${index}`">
                            <td>
                                <span class="cell-model">
                                    <img v-if="providerIcon(item.provider)" class="model-avatar" :src="providerIcon(item.provider)" :alt="item.model">
                                    <Sparkles v-else class="model-icon" :size="16" />
                                    {{ item.model }}
                                </span>
                            </td>
                            <td>{{ item.display }}</td>
                            <td><span class="vision-tag" :class="{ on: item.vision }">{{ item.vision ? '支持' : '不支持' }}</span></td>
                            <td class="col-op">
                                <Button variant="ghost" size="icon-sm" aria-label="编辑模型" @click="onEdit(index, item)"><Pencil :size="14" /></Button>
                                <Button variant="ghost" size="icon-sm" aria-label="删除模型" @click="onDelete(index, item)"><Trash2 :size="14" /></Button>
                                <Switch :model-value="item.enabled !== false" @update:model-value="(value) => onToggle(index, value)" />
                            </td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>
        <Dialog v-model:open="editorOpen">
            <DialogContent class="w-[520px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>{{ editIndex === -1 ? '添加模型' : '编辑模型' }}</DialogTitle>
                    <DialogDescription class="sr-only">配置服务商、模型名、API Key 与 Base URL</DialogDescription>
                </DialogHeader>
                <div class="form-list">
                    <div class="form-row">
                        <Label class="form-label">服务商</Label>
                        <Select v-model="form.provider" @update:model-value="onProviderChange">
                            <SelectTrigger class="form-full"><SelectValue /></SelectTrigger>
                            <SelectContent>
                                <SelectItem v-for="provider in providers" :key="provider.value" :value="provider.value">{{ provider.display }}</SelectItem>
                            </SelectContent>
                        </Select>
                    </div>
                    <div class="form-row">
                        <Label class="form-label">模型</Label>
                        <Input v-model="form.model" class="form-full" list="model-options" placeholder="选择或输入模型名" />
                        <datalist id="model-options">
                            <option v-for="name in currentModels" :key="name" :value="name" />
                        </datalist>
                    </div>
                    <div class="form-row">
                        <Label class="form-label">API Key</Label>
                        <Input v-model="form.api_key" class="form-full" type="password" :spellcheck="false" placeholder="服务商控制台申请的 API key" />
                    </div>
                    <div class="form-row">
                        <Label class="form-label">Base URL</Label>
                        <Input v-model="form.base_url" class="form-full" :spellcheck="false" placeholder="OpenAI 兼容接口地址" />
                    </div>
                </div>
                <DialogFooter>
                    <Button variant="outline" size="sm" @click="editorOpen = false">取消</Button>
                    <Button size="sm" :disabled="saving" @click="onSave">保存</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { Pencil, Plus, RefreshCw, Sparkles, Trash2 } from 'lucide-vue-next'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { Input } from '../../components/shadcn_ui/ui/input'
import { Label } from '../../components/shadcn_ui/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../components/shadcn_ui/ui/select'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../components/shadcn_ui/lib/useConfirm'
import { createModel, deleteModel, listModels, setModelEnabled, updateModel } from '../../api/request_data'
import { providerIcon } from '../../assets/providers'
const props = defineProps({ autoOpen: { type: Boolean, default: false } })
const emit = defineEmits(['opened'])
const items = ref([])
const providers = ref([])
const loading = ref(false)
const loadError = ref('')
const saving = ref(false)
const editorOpen = ref(false)
const editIndex = ref(-1)
const form = reactive({ provider: '', model: '', api_key: '', base_url: '' })
const currentModels = computed(() => (providers.value.find((provider) => provider.value === form.provider) || { models: [] }).models)
const loadList = async () => {
    loading.value = true
    try {
        const data = await listModels()
        items.value = data.list || []
        providers.value = data.providers || []
        loadError.value = ''
    } catch (err) {
        items.value = []
        loadError.value = `读取模型失败：${err.message}`
    } finally {
        loading.value = false
    }
}
const onProviderChange = (value) => {
    const meta = providers.value.find((provider) => provider.value === value)
    if (meta) {
        form.base_url = meta.base_url || ''
        form.model = (meta.models || [])[0] || ''
    }
}
const onCreate = () => {
    editIndex.value = -1
    const provider = (providers.value[0] || {}).value || ''
    Object.assign(form, { provider, model: '', api_key: '', base_url: '' })
    onProviderChange(provider)
    editorOpen.value = true
}
const onEdit = (index, item) => {
    editIndex.value = index
    Object.assign(form, { provider: item.provider, model: item.model, api_key: item.api_key || '', base_url: item.base_url || '' })
    editorOpen.value = true
}
const onSave = async () => {
    if (!form.model.trim()) { toast.warning('请选择或输入模型名'); return }
    saving.value = true
    try {
        if (editIndex.value === -1) await createModel({ ...form })
        else await updateModel(editIndex.value, { ...form })
        editorOpen.value = false
        toast.success('已保存模型')
        await loadList()
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    } finally {
        saving.value = false
    }
}
const onDelete = async (index, item) => {
    const ok = await confirm({ title: '删除模型', description: `删除模型「${item.model}」？`, confirmText: '删除', destructive: true })
    if (!ok) return
    try {
        await deleteModel(index)
        toast.success('已删除模型')
        await loadList()
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const onToggle = async (index, value) => {
    try {
        await setModelEnabled(index, value)
        await loadList()
    } catch (err) {
        toast.error(`切换失败：${err.message}`)
        await loadList()
    }
}
watch(() => props.autoOpen, (value) => {
    if (!value) return
    onCreate()
    emit('opened')
}, { immediate: true })
onMounted(loadList)
</script>

<style scoped>
.model-edit { display: flex; flex-direction: column; gap: 12px; min-height: 0; }
.model-head { display: flex; align-items: center; justify-content: space-between; }
.model-title { font-size: 15px; font-weight: 600; }
.model-card { display: flex; flex-direction: column; gap: 0; padding: 0; }
.model-top { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 14px 16px 12px; }
.model-desc { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.model-desc-text { font-size: 12px; color: var(--muted-foreground); }
.model-list { padding: 0 8px 8px; max-height: 380px; overflow-y: auto; }
.model-table { width: 100%; border-collapse: collapse; }
.model-table th { text-align: left; padding: 8px 12px; font-size: 12px; font-weight: 600; color: var(--muted-foreground); border-bottom: 1px solid var(--border); }
.model-table td { padding: 9px 12px; font-size: 13px; border-bottom: 1px solid var(--border); }
.model-table tr:hover td { background: var(--accent); }
.cell-model { display: inline-flex; align-items: center; gap: 8px; font-weight: 600; }
.model-avatar { flex: none; width: 16px; height: 16px; border-radius: 3px; object-fit: contain; }
.model-icon { color: var(--primary); }
.vision-tag { padding: 1px 7px; border-radius: 8px; font-size: 11px; color: var(--muted-foreground); background: var(--accent); }
.vision-tag.on { color: #67c23a; background: color-mix(in srgb, #67c23a 12%, transparent); }
.col-op { width: 160px; text-align: right; }
td.col-op { gap: 4px; }
.model-error { padding: 20px 12px; font-size: 12px; color: var(--destructive); }
.empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 40px 16px; color: var(--muted-foreground); }
.empty-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.empty-hint { font-size: 12px; }
.form-list { display: flex; flex-direction: column; gap: 10px; }
.form-row { display: flex; align-items: center; gap: 10px; }
.form-label { flex: none; width: 82px; }
.form-full { flex: 1; width: auto; }
</style>
