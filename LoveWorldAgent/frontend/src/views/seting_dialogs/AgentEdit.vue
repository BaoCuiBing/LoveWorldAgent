<template>
    <div class="agent-edit">
        <header class="agent-head">
            <div class="head-title">
                <span class="agent-title">智能体</span>
                <Tooltip>
                    <TooltipTrigger as-child>
                        <span class="title-help" aria-label="智能体说明"><Info :size="14" /></span>
                    </TooltipTrigger>
                    <TooltipContent>自定义智能体均为子智能体：可单独干活，主要由内置的主智能体 Agent 调用</TooltipContent>
                </Tooltip>
            </div>
            <Button @click="onCreate"><Plus :size="14" /><span>创建</span></Button>
        </header>
        <div class="group-label">自定义子智能体</div>
        <div class="panel-card agent-card">
            <div v-if="loadError" class="agent-error">{{ loadError }}</div>
            <div v-else-if="!loading && !items.length" class="agent-empty" @click="onCreate">
                <span>暂无自定义子智能体</span>
                <button type="button" class="agent-empty-link">创建</button>
            </div>
            <div v-else class="agent-list">
                <div v-for="item in items" :key="item.id" class="agent-row">
                    <User class="agent-icon" :size="20" />
                    <div class="agent-main">
                        <div class="agent-name">{{ item.name }}</div>
                        <div class="agent-desc">{{ item.when || '暂无调用说明' }}</div>
                    </div>
                    <Switch :model-value="item.enabled" :disabled="item.builtin" @update:model-value="(value) => onToggle(item, value)" />
                    <span v-if="item.builtin" class="agent-builtin">内置</span>
                    <DropdownMenu v-else>
                        <DropdownMenuTrigger as-child>
                            <Button variant="ghost" size="icon-sm" class="agent-gear" aria-label="智能体操作"><Settings :size="18" /></Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                            <DropdownMenuItem @select="onRowCommand('edit', item)">编辑</DropdownMenuItem>
                            <DropdownMenuItem @select="onRowCommand('delete', item)">删除</DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
                </div>
            </div>
        </div>
        <div class="group-label">内置主智能体</div>
        <div class="panel-card agent-card">
            <div class="agent-row">
                <span class="builtin-avatar"><MessageSquare :size="14" /></span>
                <div class="agent-main">
                    <div class="agent-name">Agent</div>
                    <div class="agent-desc">主智能体：端到端执行开发任务，可调用自定义子智能体</div>
                </div>
            </div>
        </div>
        <div class="group-label">智能体运行参数</div>
        <div class="panel-card settings-card">
            <div v-for="item in SETTING_FIELDS" :key="item.key" class="setting-row">
                <div class="setting-main">
                    <div class="setting-name">{{ item.name }}</div>
                    <div class="setting-desc">{{ item.desc }}</div>
                </div>
                <div class="setting-input">
                    <Input :model-value="settingsForm[item.key]" type="number" :min="item.min" :max="item.max" :disabled="settingsSaving" class="setting-field" @change="onSettingInput(item, $event.target.value)" />
                    <span class="setting-unit">{{ item.unit }}</span>
                </div>
            </div>
        </div>
        <Dialog v-model:open="editorOpen">
            <DialogContent class="w-[580px] max-w-[calc(100vw-32px)] max-h-[calc(100vh-160px)] overflow-y-auto">
                <DialogHeader>
                    <DialogTitle>{{ mode === 'create' ? '创建子智能体' : '编辑子智能体' }}</DialogTitle>
                    <DialogDescription class="sr-only">配置子智能体名称、提示词、调用标识、调用时机与可用工具</DialogDescription>
                </DialogHeader>
                <div class="editor-form">
                    <div class="editor-field">
                        <div class="editor-label">名称<span class="field-count">{{ form.name.length }}/20</span></div>
                        <Input v-model="form.name" maxlength="20" placeholder="智能体的显示名称" />
                    </div>
                    <div class="editor-field">
                        <div class="editor-label">提示词<span class="field-count">{{ form.body.length }}/10000</span></div>
                        <Textarea v-model="form.body" :rows="6" maxlength="10000" placeholder="请输入该子智能体的角色、语气、工作流程、工具偏好及规则规范等。（选填）" />
                    </div>
                    <div class="editor-callable">
                        <div class="callable-main">
                            <div class="editor-label">可被其他智能体调用</div>
                            <div class="callable-desc">开启后由内置的主智能体 Agent 调用；关闭则只能单独使用</div>
                        </div>
                        <Switch v-model="form.callable" />
                    </div>
                    <template v-if="form.callable">
                        <div class="editor-field">
                            <div class="editor-label">英文标识</div>
                            <Input v-model="form.id" maxlength="50" :disabled="mode === 'edit'" :spellcheck="false" placeholder="被主智能体调用时的唯一英文标识，例如：project-analyzer" />
                        </div>
                        <div class="editor-field">
                            <div class="editor-label">何时调用<span class="field-count">{{ form.when.length }}/5000</span></div>
                            <Textarea v-model="form.when" :rows="3" maxlength="5000" placeholder="请描述主智能体在什么场景和时机调用该子智能体" />
                        </div>
                    </template>
                    <div class="editor-field">
                        <div class="editor-label">工具</div>
                        <div class="tool-list">
                            <div v-for="tool in toolOptions" :key="tool.value" class="tool-row" :class="{ disabled: toolDisabled(tool.value) }" @click="!toolDisabled(tool.value) && toggleTool(tool.value)">
                                <Checkbox :model-value="toolChecks[tool.value]" :disabled="toolDisabled(tool.value)" @click.stop @update:model-value="(value) => { toolChecks[tool.value] = value }" />
                                <component :is="tool.icon" class="tool-icon" :size="16" />
                                <span class="tool-main"><span class="tool-name">{{ tool.name }}</span><span class="tool-desc">{{ tool.desc }}</span></span>
                            </div>
                        </div>
                        <div class="tool-hint">全不选时默认启用全部工具</div>
                    </div>
                </div>
                <DialogFooter>
                    <Button variant="outline" size="sm" @click="editorOpen = false">取消</Button>
                    <Button size="sm" :disabled="saving" @click="onSave">{{ mode === 'create' ? '创建' : '保存' }}</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { onMounted, reactive, ref, watch } from 'vue'
import { BookOpen, Cpu, FileText, Folder, Info, MessageSquare, Monitor, Plus, Settings, SquareTerminal, User } from 'lucide-vue-next'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Checkbox } from '../../components/shadcn_ui/ui/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../../components/shadcn_ui/ui/dropdown-menu'
import { Input } from '../../components/shadcn_ui/ui/input'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { Textarea } from '../../components/shadcn_ui/ui/textarea'
import { Tooltip, TooltipContent, TooltipTrigger } from '../../components/shadcn_ui/ui/tooltip'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../components/shadcn_ui/lib/useConfirm'
import { createAgent, deleteAgent, getAgent, getAgentSettings, getBrowserSettings, getComputerUse, listAgents, saveAgentSettings, setAgentEnabled, updateAgent } from '../../api/request_data'
const props = defineProps({ autoOpen: { type: Boolean, default: false } })
const emit = defineEmits(['opened'])
const ID_RE = /^[a-z][a-z0-9-]{1,49}$/
const SETTING_FIELDS = [
    { key: 'max_turns', name: '单轮最大工具轮次', unit: '轮', min: 1, max: 500, desc: '一次对话允许连续推进的工具调用轮数。过小会让长任务频繁截断并需人工点「继续」，保存后下一轮对话即刻生效。' },
    { key: 'timeout_seconds', name: '模型请求超时', unit: '秒', min: 10, max: 600, desc: '单次请求大模型的等待上限，包含流式返回全过程。' },
    { key: 'command_timeout_seconds', name: '命令执行超时', unit: '秒', min: 5, max: 600, desc: 'command_run 与 terminal_send 等一条命令的等待上限。' },
]
const toolOptions = [
    { value: 'rule', name: '阅读', desc: '对规则进行检索和读写', icon: FileText },
    { value: 'skill', name: '技能', desc: '读取已启用技能', icon: BookOpen },
    { value: 'file', name: '文件', desc: '对项目文件进行读写删', icon: Folder },
    { value: 'command', name: '命令', desc: '执行 shell 命令（受白名单约束）', icon: SquareTerminal },
    { value: 'browser', name: '浏览器', desc: '驱动内置浏览器完成网页任务', icon: Monitor },
    { value: 'computer', name: '电脑控制', desc: '操作桌面键鼠与窗口', icon: Cpu },
]
const items = ref([])
const loading = ref(false)
const loadError = ref('')
const saving = ref(false)
const editorOpen = ref(false)
const mode = ref('create')
const editId = ref('')
const form = reactive({ name: '', body: '', callable: false, id: '', when: '' })
const toolChecks = reactive({ rule: true, skill: true, file: true, command: true, browser: false, computer: false })
const browserEnabled = ref(true)
const computerEnabled = ref(true)
const settingsForm = reactive({ max_turns: 200, timeout_seconds: 180, command_timeout_seconds: 60 })
const settingsSaving = ref(false)
const toolDisabled = (value) => (value === 'browser' && !browserEnabled.value) || (value === 'computer' && !computerEnabled.value)
const toggleTool = (value) => {
    if (toolDisabled(value)) return
    toolChecks[value] = !toolChecks[value]
}
const loadBrowserEnabled = async () => {
    try {
        const data = await getBrowserSettings()
        browserEnabled.value = data.enabled !== false
    } catch { /* 读取失败保持启用 */ }
    if (!browserEnabled.value) toolChecks.browser = false
}
const loadComputerEnabled = async () => {
    try {
        const data = await getComputerUse()
        computerEnabled.value = Boolean(data && data.enabled)
    } catch { /* 读取失败保持启用 */ }
    if (!computerEnabled.value) toolChecks.computer = false
}
const loadSettings = async () => {
    try {
        const data = await getAgentSettings()
        SETTING_FIELDS.forEach((item) => { settingsForm[item.key] = Number(data[item.key]) || settingsForm[item.key] })
    } catch { /* 读取失败保持默认 */ }
}
const onSettingInput = (item, raw) => {
    const value = Math.min(item.max, Math.max(item.min, Math.round(Number(raw) || item.min)))
    settingsForm[item.key] = value
    onSettingChange(item)
}
const onSettingChange = async (item) => {
    if (settingsSaving.value) return
    settingsSaving.value = true
    try {
        const data = await saveAgentSettings({ [item.key]: settingsForm[item.key] })
        SETTING_FIELDS.forEach((field) => { settingsForm[field.key] = Number(data[field.key]) || settingsForm[field.key] })
        toast.success('运行参数已保存')
    } catch (err) {
        toast.error(`保存运行参数失败：${err.message}`)
    } finally {
        settingsSaving.value = false
    }
}
const loadList = async () => {
    loading.value = true
    try {
        const data = await listAgents()
        items.value = data.list || []
        loadError.value = ''
    } catch (err) {
        items.value = []
        loadError.value = `读取智能体失败：${err.message}`
    } finally {
        loading.value = false
    }
}
const resetTools = (tools) => {
    const list = Array.isArray(tools) && tools.length ? tools : ['rule', 'skill', 'file', 'command']
    toolChecks.rule = list.includes('rule')
    toolChecks.skill = list.includes('skill')
    toolChecks.file = list.includes('file')
    toolChecks.command = list.includes('command')
    toolChecks.browser = list.includes('browser')
    toolChecks.computer = list.includes('computer')
    if (!browserEnabled.value) toolChecks.browser = false
    if (!computerEnabled.value) toolChecks.computer = false
}
const selectedTools = () => toolOptions.filter((tool) => toolChecks[tool.value] && !toolDisabled(tool.value)).map((tool) => tool.value)
const onCreate = () => {
    mode.value = 'create'
    editId.value = ''
    Object.assign(form, { name: '', body: '', callable: false, id: '', when: '' })
    resetTools([])
    editorOpen.value = true
}
watch(() => props.autoOpen, (value) => {
    if (!value) return
    if (!editorOpen.value) onCreate()
    emit('opened')
}, { immediate: true })
const onEdit = async (item) => {
    let detail = item
    try {
        detail = await getAgent(item.id)
    } catch { /* 读取详情失败时回退列表项，不阻断编辑 */ }
    mode.value = 'edit'
    editId.value = item.id
    Object.assign(form, { name: detail.name || '', body: detail.body || '', callable: Boolean(detail.callable), id: detail.id || item.id, when: detail.when || '' })
    resetTools(detail.tools)
    editorOpen.value = true
}
const onDelete = async (item) => {
    const ok = await confirm({ title: '删除智能体', description: `删除子智能体「${item.name}」？该操作不可撤销。`, confirmText: '删除', destructive: true })
    if (!ok) return
    try {
        await deleteAgent(item.id)
        toast.success('已删除智能体')
        await loadList()
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const onRowCommand = (command, item) => (command === 'edit' ? onEdit(item) : onDelete(item))
const onToggle = async (item, value) => {
    if (item.builtin) return
    const previous = item.enabled
    item.enabled = value
    try {
        await setAgentEnabled(item.id, value)
    } catch (err) {
        item.enabled = previous
        toast.error(`切换失败：${err.message}`)
    }
}
const onSave = async () => {
    if (!form.name.trim()) { toast.warning('请填写智能体名称'); return }
    const idValue = form.id.trim()
    if (form.callable && !ID_RE.test(idValue)) { toast.warning('英文标识需以小写字母开头，仅含小写字母、数字与横线（2-50 位）'); return }
    saving.value = true
    try {
        const payload = { name: form.name, body: form.body, callable: form.callable, when: form.callable ? form.when : '', tools: selectedTools() }
        if (mode.value === 'create') await createAgent({ id: idValue || `agent-${Date.now()}`, ...payload })
        else await updateAgent(editId.value, payload)
        editorOpen.value = false
        toast.success(mode.value === 'create' ? '已创建子智能体' : '已保存子智能体')
        await loadList()
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    } finally {
        saving.value = false
    }
}
onMounted(() => { loadList(); loadSettings(); loadBrowserEnabled(); loadComputerEnabled() })
</script>

<style scoped>
.agent-edit { display: flex; flex-direction: column; gap: 8px; min-height: 0; }
.agent-head { display: flex; align-items: center; justify-content: space-between; }
.head-title { display: flex; align-items: center; gap: 6px; }
.agent-title { font-size: 15px; font-weight: 600; }
.title-help { display: inline-flex; align-items: center; color: var(--muted-foreground); cursor: help; }
.group-label { font-size: 12px; color: var(--muted-foreground); }
.agent-card { display: flex; flex-direction: column; padding: 6px; }
.agent-list { display: flex; flex-direction: column; gap: 2px; max-height: 300px; overflow-y: auto; }
.agent-row { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: 6px; }
.agent-row:hover { background: var(--accent); }
.agent-icon { flex: none; box-sizing: content-box; padding: 8px; border-radius: 8px; color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.builtin-avatar { display: inline-flex; align-items: center; justify-content: center; width: 36px; height: 36px; flex: none; border-radius: 8px; color: #67c23a; background: color-mix(in srgb, #67c23a 12%, transparent); }
.agent-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.agent-name { font-size: 13px; font-weight: 600; }
.agent-desc { overflow: hidden; font-size: 12px; color: var(--muted-foreground); text-overflow: ellipsis; white-space: nowrap; }
.agent-gear { flex: none; cursor: pointer; }
.agent-builtin { flex: none; padding: 1px 6px; border-radius: 8px; background: var(--muted); color: var(--muted-foreground); font-size: 11px; }
.agent-empty { display: flex; align-items: center; justify-content: center; gap: 8px; padding: 24px 12px; font-size: 12px; color: var(--muted-foreground); cursor: pointer; border-radius: 6px; }
.agent-empty:hover { background: var(--accent); }
.agent-empty-link { padding: 0; border: none; background: transparent; color: var(--primary); font-size: 12px; cursor: pointer; }
.agent-empty-link:hover { text-decoration: underline; }
.agent-error { padding: 20px 12px; font-size: 12px; color: var(--destructive); }
.editor-form { display: flex; flex-direction: column; gap: 16px; }
.editor-field { display: flex; flex-direction: column; gap: 6px; }
.editor-label { display: flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; }
.field-count { font-size: 11px; font-weight: 400; color: var(--muted-foreground); }
.editor-callable { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 12px; border: 1px solid var(--border); border-radius: 8px; }
.callable-main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.callable-desc { font-size: 12px; color: var(--muted-foreground); }
.tool-list { display: flex; flex-direction: column; gap: 8px; }
.tool-row { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px; cursor: pointer; }
.tool-row:hover { background: var(--accent); }
.tool-row.disabled { opacity: 0.55; cursor: not-allowed; }
.tool-row.disabled:hover { background: transparent; }
.tool-icon { color: var(--primary); }
.tool-main { display: flex; flex-direction: column; gap: 2px; }
.tool-name { font-size: 13px; color: var(--foreground); }
.tool-desc { font-size: 11px; color: var(--muted-foreground); }
.tool-hint { margin-top: 6px; font-size: 11px; color: var(--muted-foreground); }
.settings-card { display: flex; flex-direction: column; padding: 0; }
.setting-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 14px; }
.setting-row + .setting-row { border-top: 1px solid var(--border); }
.setting-main { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.setting-name { font-size: 13px; font-weight: 600; }
.setting-desc { font-size: 12px; line-height: 1.5; color: var(--muted-foreground); }
.setting-input { position: relative; width: 160px; flex: none; }
.setting-field { padding-right: 28px; }
.setting-unit { position: absolute; right: 8px; top: 50%; transform: translateY(-50%); font-size: 11px; color: var(--muted-foreground); pointer-events: none; }
</style>
