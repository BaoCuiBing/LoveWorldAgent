<template>
    <div class="skill-edit">
        <header class="skill-head">
            <span class="skill-title">技能</span>
            <Button variant="ghost" size="icon-sm" aria-label="刷新技能列表" :disabled="loading" @click="loadList"><RefreshCw :size="18" :class="{ 'animate-spin': loading }" /></Button>
        </header>
        <div class="skill-search-wrap">
            <Search class="skill-search-icon" :size="14" />
            <Input v-model="keyword" class="skill-search" placeholder="搜索技能" />
            <button v-if="keyword" type="button" class="skill-search-clear" aria-label="清空搜索" @click="keyword = ''"><X :size="12" /></button>
        </div>
        <div class="skill-card panel-card">
            <div class="skill-top">
                <div class="skill-desc">
                    <div class="skill-desc-title">技能</div>
                    <div class="skill-desc-text">技能是一组基于上下文触发的指令集，用于稳定地执行任务。<button type="button" class="skill-more" @click="onMore">了解更多</button></div>
                </div>
                <DropdownMenu>
                    <DropdownMenuTrigger as-child>
                        <Button><Plus :size="14" /><span>创建</span><ChevronDown :size="14" /></Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                        <DropdownMenuItem @select="onCreate('blank')">新建技能</DropdownMenuItem>
                        <DropdownMenuItem @select="onCreate('template')">从模板新建</DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>
            <div class="skill-list">
                <div v-if="loadError" class="skill-error">{{ loadError }}</div>
                <div v-else-if="!loading && !items.length && !keyword.trim()" class="empty-state">
                    <BookOpen :size="36" />
                    <div class="empty-title">暂无技能</div>
                    <div class="empty-hint">点击新建以添加你的第一个技能</div>
                </div>
                <div v-for="item in items" :key="item.name" class="skill-row" :title="item.name">
                    <BookOpen class="skill-icon" :size="20" />
                    <div class="skill-main">
                        <div class="skill-name"><template v-for="(part, index) in highlight(item.name)" :key="index"><span :class="{ hit: part.hit }">{{ part.text }}</span></template></div>
                        <div class="skill-text"><template v-for="(part, index) in highlight(item.description || '（未填写描述）')" :key="index"><span :class="{ hit: part.hit }">{{ part.text }}</span></template></div>
                        <div v-if="item.locked" class="skill-locked">随{{ driverLabel(item.driven_by) }}开关</div>
                    </div>
                    <Switch :model-value="item.enabled" :disabled="item.locked" @update:model-value="(value) => onToggle(item, value)" />
                    <DropdownMenu>
                        <DropdownMenuTrigger as-child>
                            <Button variant="ghost" size="icon-sm" class="skill-gear" :disabled="item.locked" aria-label="技能操作"><Settings :size="18" /></Button>
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
            <DialogContent class="w-[680px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>{{ mode === 'create' ? '新建技能' : '编辑技能' }}</DialogTitle>
                    <DialogDescription class="sr-only">编辑技能名称、用途描述、启用状态与指令内容</DialogDescription>
                </DialogHeader>
                <div class="editor-form">
                    <Input v-if="mode === 'create'" v-model="form.name" maxlength="41" :spellcheck="false" placeholder="技能文件夹名，例如 code-review（字母、数字、-_.）" />
                    <span v-else class="editor-name">{{ form.name }}</span>
                    <Input v-model="form.description" maxlength="200" placeholder="技能用途：什么场景下触发、解决什么问题" />
                    <div class="editor-enabled">
                        <span>{{ form.locked ? `启用该技能（随${driverLabel(form.driven_by)}开关，不可手动修改）` : '启用该技能' }}</span>
                        <Switch v-model="form.enabled" :disabled="form.locked" />
                    </div>
                    <CodeEditor v-model="form.content" height="260px" />
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
import { onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { BookOpen, ChevronDown, Plus, RefreshCw, Search, Settings, X } from 'lucide-vue-next'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../../components/shadcn_ui/ui/dropdown-menu'
import { Input } from '../../components/shadcn_ui/ui/input'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../components/shadcn_ui/lib/useConfirm'
import CodeEditor from '../../components/editor/CodeEditor.vue'
import { createSkill, deleteSkill, listSkills, openDoc, readSkill, setSkillEnabled, updateSkill } from '../../api/request_data'
const TEMPLATE_DESCRIPTION = '说明该技能在什么场景下使用、解决什么问题。'
const TEMPLATE_SKILL = '# 技能标题\n\n## 使用场景\n\n- 描述触发该技能的任务类型。\n\n## 执行步骤\n\n1. 第一步可执行的操作。\n2. 第二步可执行的操作。\n\n## 注意事项\n\n- 需要避免的错误做法。\n'
const DRIVER_LABELS = { browser: '浏览器', computer: '电脑', word: 'Word', ppt: 'PPT', excel: 'Excel', mobile: '移动端' }
const driverLabel = (driver) => DRIVER_LABELS[String(driver || '')] || '内置工具'
const items = ref([])
const keyword = ref('')
const loading = ref(false)
const loadError = ref('')
const saving = ref(false)
const editorOpen = ref(false)
const mode = ref('create')
const form = reactive({ name: '', description: '', content: '', enabled: true, locked: false, driven_by: '' })
let searchTimer = 0
let loadSeq = 0
const highlight = (text) => {
    const source = String(text || '')
    const word = keyword.value.trim().toLowerCase()
    if (!word) return [{ text: source, hit: false }]
    const lower = source.toLowerCase()
    const parts = []
    let index = 0
    let hit = lower.indexOf(word)
    while (hit !== -1) {
        if (hit > index) parts.push({ text: source.slice(index, hit), hit: false })
        parts.push({ text: source.slice(hit, hit + word.length), hit: true })
        index = hit + word.length
        hit = lower.indexOf(word, index)
    }
    if (index < source.length) parts.push({ text: source.slice(index), hit: false })
    return parts.length ? parts : [{ text: source, hit: false }]
}
const loadList = async () => {
    const seq = ++loadSeq
    loading.value = true
    try {
        const data = await listSkills(keyword.value.trim())
        if (seq !== loadSeq) return
        items.value = data.list || []
        loadError.value = ''
    } catch (err) {
        if (seq !== loadSeq) return
        items.value = []
        loadError.value = `读取技能失败：${err.message}`
    } finally {
        if (seq === loadSeq) loading.value = false
    }
}
watch(keyword, () => {
    clearTimeout(searchTimer)
    searchTimer = setTimeout(loadList, 300)
})
onBeforeUnmount(() => clearTimeout(searchTimer))
const onCreate = (command) => {
    mode.value = 'create'
    form.name = ''
    form.description = command === 'template' ? TEMPLATE_DESCRIPTION : ''
    form.content = command === 'template' ? TEMPLATE_SKILL : ''
    form.enabled = true
    form.locked = false
    form.driven_by = ''
    editorOpen.value = true
}
const onEdit = async (item) => {
    try {
        const data = await readSkill(item.name)
        mode.value = 'edit'
        form.name = data.name
        form.description = data.description || ''
        form.content = data.body || ''
        form.enabled = Boolean(data.enabled)
        form.locked = Boolean(data.locked)
        form.driven_by = data.driven_by || ''
        editorOpen.value = true
    } catch (err) {
        toast.error(`读取技能失败：${err.message}`)
    }
}
const onDelete = async (item) => {
    const ok = await confirm({ title: '删除技能', description: `删除技能「${item.name}」？技能目录及其附件将一并删除。`, confirmText: '删除', destructive: true })
    if (!ok) return
    try {
        await deleteSkill(item.name)
        toast.success('已删除技能')
        await loadList()
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const onRowCommand = (command, item) => (command === 'edit' ? onEdit(item) : onDelete(item))
const onToggle = async (item, value) => {
    const previous = item.enabled
    item.enabled = value
    try {
        await setSkillEnabled(item.name, value)
    } catch (err) {
        item.enabled = previous
        toast.error(`切换失败：${err.message}`)
    }
}
const onSave = async () => {
    if (mode.value === 'create' && !form.name.trim()) { toast.warning('请填写技能文件夹名'); return }
    saving.value = true
    try {
        const payload = { description: form.description, content: form.content, enabled: form.enabled }
        if (mode.value === 'create') await createSkill({ name: form.name, ...payload })
        else await updateSkill(form.name, payload)
        editorOpen.value = false
        toast.success('已保存技能')
        await loadList()
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    } finally {
        saving.value = false
    }
}
const onMore = () => openDoc('skills.md').catch((err) => toast.error(`打开文档失败：${err.message}`))
onMounted(loadList)
</script>

<style scoped>
.skill-edit { display: flex; flex-direction: column; gap: 12px; min-height: 0; }
.skill-head { display: flex; align-items: center; justify-content: space-between; }
.skill-title { font-size: 15px; font-weight: 600; }
.skill-search-wrap { position: relative; flex: none; }
.skill-search { padding-left: 26px; padding-right: 24px; }
.skill-search-icon { position: absolute; left: 8px; top: 50%; transform: translateY(-50%); color: var(--muted-foreground); pointer-events: none; }
.skill-search-clear { position: absolute; right: 6px; top: 50%; display: inline-flex; align-items: center; justify-content: center; width: 16px; height: 16px; padding: 0; border: none; border-radius: 50%; background: var(--muted); color: var(--muted-foreground); transform: translateY(-50%); cursor: pointer; }
.skill-search-clear:hover { color: var(--foreground); }
.skill-card { display: flex; flex-direction: column; gap: 0; padding: 0; }
.skill-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; min-height: 82px; padding: 20px 16px 18px; }
.skill-desc { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.skill-desc-title { font-size: 14px; font-weight: 600; }
.skill-desc-text { font-size: 12px; color: var(--muted-foreground); }
.skill-list { display: flex; flex-direction: column; gap: 2px; max-height: 300px; padding: 0 8px 8px; overflow-y: auto; }
.skill-row { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: 6px; }
.skill-row:not(:last-child) { border-bottom: 1px solid var(--border); }
.skill-row:hover { background: var(--accent); }
.skill-icon { flex: none; box-sizing: content-box; padding: 8px; border-radius: 8px; color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.skill-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.skill-name { font-size: 13px; font-weight: 600; }
.skill-text { display: -webkit-box; overflow: hidden; font-size: 12px; color: var(--muted-foreground); -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
.hit { color: var(--primary); font-weight: 600; }
.skill-gear { flex: none; cursor: pointer; }
.skill-empty { padding: 20px 12px; }
.skill-error { padding: 20px 12px; font-size: 12px; color: var(--destructive); }
.empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 40px 16px; color: var(--muted-foreground); }
.empty-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.empty-hint { font-size: 12px; }
.skill-more { padding: 0; border: none; background: transparent; color: var(--primary); font-size: 12px; vertical-align: baseline; cursor: pointer; }
.skill-more:hover { text-decoration: underline; }
.skill-locked { font-size: 11px; color: var(--muted-foreground); margin-top: 2px; }
.editor-form { display: flex; flex-direction: column; gap: 10px; }
.editor-name { display: inline-flex; align-items: center; gap: 6px; }
.editor-enabled { display: flex; align-items: center; justify-content: space-between; font-size: 13px; }
</style>
