<template>
    <div class="relus-edit">
        <header class="relus-head">
            <span class="relus-title">规则</span>
            <Button variant="ghost" size="icon-sm" aria-label="刷新规则列表" :disabled="loading" @click="loadList"><RefreshCw :size="18" :class="{ 'animate-spin': loading }" /></Button>
        </header>
        <div class="relus-card panel-card">
            <div class="relus-top">
                <div class="relus-desc">
                    <div class="relus-desc-title">规则</div>
                    <div class="relus-desc-text">创建并管理规则，在聊天过程中遵循这些规则。<button type="button" class="relus-more" @click="onMore">了解更多</button></div>
                </div>
                <DropdownMenu>
                    <DropdownMenuTrigger as-child>
                        <Button><Plus :size="14" /><span>创建</span><ChevronDown :size="14" /></Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                        <DropdownMenuItem @select="onCreate('blank')">新建规则</DropdownMenuItem>
                        <DropdownMenuItem @select="onCreate('template')">从模板新建</DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>
            <div class="relus-list">
                <div v-if="loadError" class="relus-error">{{ loadError }}</div>
                <div v-else-if="!loading && !items.length" class="empty-state">
                    <FileText :size="36" />
                    <div class="empty-title">暂无规则</div>
                    <div class="empty-hint">点击新建以添加你的第一个规则</div>
                </div>
                <div v-for="item in items" :key="item.name" class="relus-row" :title="item.name">
                    <FileText class="relus-icon" :size="20" />
                    <div class="relus-main">
                        <div class="relus-name">{{ item.name }}<Badge variant="outline">{{ modeLabel(item.trigger) }}</Badge></div>
                        <div class="relus-summary">{{ item.preview || '（空规则）' }}</div>
                    </div>
                    <DropdownMenu>
                        <DropdownMenuTrigger as-child>
                            <Button variant="ghost" size="icon-sm" class="relus-gear" aria-label="规则操作"><Settings :size="18" /></Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                            <DropdownMenuItem @select="onRowCommand('edit', item)">编辑</DropdownMenuItem>
                            <DropdownMenuItem @select="onRowCommand('delete', item)">删除</DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
                </div>
            </div>
        </div>
        <Dialog v-model:open="editorOpen">
            <DialogContent class="w-[640px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>{{ mode === 'create' ? '新建规则' : '编辑规则' }}</DialogTitle>
                    <DialogDescription class="sr-only">编辑规则内容、生效方式与描述</DialogDescription>
                </DialogHeader>
                <div class="editor-form">
                    <Input v-if="mode === 'create'" v-model="form.name" maxlength="60" :spellcheck="false" placeholder="规则文件名，例如 code-style（自动补全 .md）" />
                    <span v-else class="editor-name">{{ form.name }}</span>
                    <div class="editor-row">
                        <span class="editor-label">生效方式</span>
                        <Select v-model="form.applyMode">
                            <SelectTrigger class="editor-select"><SelectValue /></SelectTrigger>
                            <SelectContent>
                                <SelectItem value="model_decision">智能生效</SelectItem>
                                <SelectItem value="always_on">始终生效</SelectItem>
                            </SelectContent>
                        </Select>
                        <Input v-if="form.applyMode === 'model_decision'" v-model="form.description" class="editor-input" maxlength="60" placeholder="描述" />
                    </div>
                    <div class="editor-switch">
                        <Tabs v-model="view">
                            <TabsList>
                                <TabsTrigger value="edit">编辑</TabsTrigger>
                                <TabsTrigger value="preview">预览</TabsTrigger>
                            </TabsList>
                        </Tabs>
                        <span class="editor-count">{{ form.content.length }} 字</span>
                    </div>
                    <CodeEditor v-if="view === 'edit'" v-model="form.content" height="300px" />
                    <div v-else class="editor-preview">{{ form.content }}</div>
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
import { onMounted, reactive, ref } from 'vue'
import { ChevronDown, FileText, Plus, RefreshCw, Settings } from 'lucide-vue-next'
import { Badge } from '../../components/shadcn_ui/ui/badge'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../../components/shadcn_ui/ui/dropdown-menu'
import { Input } from '../../components/shadcn_ui/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../components/shadcn_ui/ui/select'
import { Tabs, TabsList, TabsTrigger } from '../../components/shadcn_ui/ui/tabs'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../components/shadcn_ui/lib/useConfirm'
import CodeEditor from '../../components/editor/CodeEditor.vue'
import { createRelu, deleteRelu, listRelus, openDoc, readRelu, updateRelu } from '../../api/request_data'
const TEMPLATE_RULE = '# 规则标题\n\n1. 第一条可执行的约束。\n2. 第二条可执行的约束。\n'
const items = ref([])
const loading = ref(false)
const loadError = ref('')
const saving = ref(false)
const editorOpen = ref(false)
const mode = ref('create')
const view = ref('edit')
const form = reactive({ name: '', content: '', applyMode: 'model_decision', description: '' })
const modeLabel = (trigger) => (trigger === 'always_on' ? '始终生效' : '智能生效')
const loadList = async () => {
    loading.value = true
    try {
        const data = await listRelus()
        items.value = data.list || []
        loadError.value = ''
    } catch (err) {
        items.value = []
        loadError.value = `读取规则失败：${err.message}`
    } finally {
        loading.value = false
    }
}
const onCreate = (command) => {
    mode.value = 'create'
    view.value = 'edit'
    form.name = ''
    form.content = command === 'template' ? TEMPLATE_RULE : ''
    form.applyMode = 'model_decision'
    form.description = ''
    editorOpen.value = true
}
const onEdit = async (item) => {
    try {
        const data = await readRelu(item.name)
        mode.value = 'edit'
        view.value = 'edit'
        form.name = data.name
        form.content = data.body || ''
        form.applyMode = data.trigger === 'always_on' ? 'always_on' : 'model_decision'
        form.description = data.description || ''
        editorOpen.value = true
    } catch (err) {
        toast.error(`读取规则失败：${err.message}`)
    }
}
const onDelete = async (item) => {
    const ok = await confirm({ title: '删除规则', description: `删除规则「${item.name}」？该操作不可撤销。`, confirmText: '删除', destructive: true })
    if (!ok) return
    try {
        await deleteRelu(item.name)
        toast.success('已删除规则')
        await loadList()
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const onRowCommand = (command, item) => (command === 'edit' ? onEdit(item) : onDelete(item))
const onSave = async () => {
    if (mode.value === 'create' && !form.name.trim()) { toast.warning('请填写规则文件名'); return }
    saving.value = true
    try {
        if (mode.value === 'create') await createRelu(form.name, form.content, form.applyMode, form.description)
        else await updateRelu(form.name, form.content, form.applyMode, form.description)
        editorOpen.value = false
        toast.success('已保存规则')
        await loadList()
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    } finally {
        saving.value = false
    }
}
const onMore = () => openDoc('rules.md').catch((err) => toast.error(`打开文档失败：${err.message}`))
onMounted(loadList)
</script>

<style scoped>
.relus-edit { display: flex; flex-direction: column; gap: 12px; min-height: 0; min-width: 0; max-width: 100%; overflow: hidden; }
.relus-head { display: flex; align-items: center; justify-content: space-between; }
.relus-title { font-size: 15px; font-weight: 600; }
.relus-card { display: flex; flex-direction: column; gap: 0; padding: 0; min-width: 0; max-width: 100%; overflow: hidden; }
.relus-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; padding: 14px 16px 12px; min-width: 0; max-width: 100%; overflow: hidden; }
.relus-desc { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.relus-desc-title { font-size: 14px; font-weight: 600; }
.relus-desc-text { font-size: 12px; color: var(--muted-foreground); }
.relus-list { display: flex; flex-direction: column; gap: 2px; max-height: 300px; padding: 0 8px 8px; overflow-y: auto; overflow-x: hidden; min-width: 0; max-width: 100%; }
.relus-row { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: 6px; min-width: 0; max-width: 100%; overflow: hidden; }
.relus-row:not(:last-child) { border-bottom: 1px solid var(--border); }
.relus-row:hover { background: var(--accent); }
.relus-icon { flex: none; box-sizing: content-box; padding: 8px; border-radius: 8px; color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.relus-main { flex: 1; min-width: 0; max-width: 100%; overflow: hidden; display: flex; flex-direction: column; gap: 2px; }
.relus-name { display: flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; min-width: 0; }
.relus-summary { overflow: hidden; font-size: 12px; color: var(--muted-foreground); text-overflow: ellipsis; white-space: nowrap; min-width: 0; max-width: 100%; }
.relus-gear { flex: none; cursor: pointer; }
.relus-empty { padding: 20px 12px; }
.relus-error { padding: 20px 12px; font-size: 12px; color: var(--destructive); }
.relus-more { padding: 0; border: none; background: transparent; color: var(--primary); font-size: 12px; vertical-align: baseline; cursor: pointer; }
.relus-more:hover { text-decoration: underline; }
.empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 40px 16px; color: var(--muted-foreground); }
.empty-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.empty-hint { font-size: 12px; }
.editor-form { display: flex; flex-direction: column; gap: 10px; }
.editor-row { display: flex; align-items: center; gap: 10px; }
.editor-label { flex: none; font-size: 13px; font-weight: 600; }
.editor-select { flex: none; width: 120px; }
.editor-row .editor-input { flex: 1; }
.editor-name { display: inline-flex; align-items: center; gap: 6px; }
.editor-switch { display: flex; align-items: center; justify-content: space-between; }
.editor-preview { height: 300px; overflow-y: auto; white-space: pre-wrap; }
</style>
