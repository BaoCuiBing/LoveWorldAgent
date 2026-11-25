<template>
    <div class="input-message">
        <InsertContext v-model:visible="panelOpen" :mode="panelMode" :root-path="projectPath" @select="onPanelSelect" @manage="onManageSkills" />
        <Popover v-model:open="agentTriggerOpen">
            <PopoverAnchor as-child>
                <span ref="agentAnchorRef" class="agent-anchor" />
            </PopoverAnchor>
            <PopoverContent side="top" align="start" :side-offset="6" class="w-[220px]">
                <div class="agent-menu" @mousedown.stop>
                    <div class="menu-label">内置智能体</div>
                    <div class="menu-item" @click="selectTriggerAgent">
                        <span class="agent-avatar"><MessageSquare :size="12" /></span>
                        <div class="item-main"><div class="item-title">Agent</div></div>
                        <Check v-if="!agentId" class="item-check" :size="14" />
                    </div>
                    <div v-for="agent in customAgents" :key="agent.id" class="menu-item" @click="selectAgent(agent)">
                        <span class="agent-avatar"><User :size="12" /></span>
                        <div class="item-main"><div class="item-title">{{ agent.name }}</div></div>
                        <Check v-if="agentId === agent.id" class="item-check" :size="14" />
                    </div>
                    <Button class="menu-block-btn" size="sm" @click="onCreateAgent">创建智能体</Button>
                </div>
            </PopoverContent>
        </Popover>
        <input ref="fileRef" class="file-input" type="file" multiple @change="onFileChange" />
        <div v-if="attachments.length" class="attach-bar">
            <div v-for="(item, index) in attachments" :key="item.path" class="attach-item" :class="{ 'is-image': item.isImage }" :title="item.name">
                <img v-if="item.isImage" class="attach-thumb" :src="item.url" :alt="item.name" />
                <template v-else>
                    <FileText class="attach-file-icon" :size="14" />
                    <span class="attach-name">{{ item.name }}</span>
                </template>
                <button class="attach-remove" type="button" aria-label="移除附件" @click="removeAttachment(index)"><X :size="10" /></button>
            </div>
        </div>
        <Popover v-model:open="pathOpen">
            <PopoverAnchor as-child>
                <span ref="pathAnchorRef" class="agent-anchor" />
            </PopoverAnchor>
            <PopoverContent side="top" align="start" :side-offset="6" class="w-[360px]">
                <div class="path-menu" @mousedown.stop>
                    <div class="menu-label">{{ pathKind === 'dir' ? 'Folder' : 'File' }}</div>
                    <div v-if="pathLoading" class="group-empty">加载中…</div>
                    <div v-else-if="pathError" class="group-empty">{{ pathError }}</div>
                    <div v-else-if="!pathItems.length" class="group-empty">没有匹配项（先在顶栏选择项目文件夹）</div>
                    <div v-for="item in pathItems" :key="item.rel" class="menu-item" :title="item.abs || item.rel" @click="pickPath(item)">
                        <component :is="item.type === 'dir' ? Folder : FileText" class="item-icon" :size="15" />
                        <span class="path-name">{{ item.name }}</span>
                        <span class="path-abs">{{ item.abs || item.rel }}</span>
                    </div>
                </div>
            </PopoverContent>
        </Popover>
        <Textarea ref="inputRef" v-model="text" rows="2" class="input-textarea" placeholder="帮你编写代码、调试 Bug、优化性能等开发工作，交付生产级代码产物。" @input="onInput" @click="syncTrigger" @keydown="onKeydown" />
        <div class="input-toolbar">
            <div class="toolbar-left">
                <Popover v-model:open="plusOpen">
                    <PopoverTrigger as-child>
                        <Button variant="ghost" size="icon" aria-label="更多操作">
                            <Plus :size="14" />
                        </Button>
                    </PopoverTrigger>
                    <PopoverContent side="top" align="start" :side-offset="6" class="w-[240px]">
                        <div class="plus-menu">
                            <div class="menu-item" @click="onAddContext">
                                <Info class="item-icon" :size="15" />
                                <div class="item-main"><div class="item-title">添加上下文</div></div>
                            </div>
                            <div class="menu-item" @click="onUploadClick">
                                <Paperclip class="item-icon" :size="15" />
                                <div class="item-main"><div class="item-title">{{ uploading ? '上传中…' : '上传附件' }}</div></div>
                            </div>
                            <div class="menu-divider" />
                            <div class="menu-item" @click="onPickSlash">
                                <Zap class="item-icon" :size="15" />
                                <div class="item-main"><div class="item-title">命令</div></div>
                            </div>
                            <div class="menu-item" @click="onPickSlash">
                                <BookOpen class="item-icon" :size="15" />
                                <div class="item-main"><div class="item-title">技能</div></div>
                            </div>
                        </div>
                    </PopoverContent>
                </Popover>
                <Popover v-model:open="approveOpen">
                    <PopoverTrigger as-child>
                        <Button variant="ghost" class="tool-chip">
                            <CircleCheck :size="14" />
                            <span>{{ approveText }}</span>
                            <ArrowUp class="chip-arrow" :class="{ open: approveOpen }" :size="14" />
                        </Button>
                    </PopoverTrigger>
                    <PopoverContent side="top" align="start" :side-offset="6" class="w-[300px]">
                        <div class="menu-head">如何批准 Agent 的操作？<Button variant="link" size="xs" @click="openPermissionDoc">了解更多</Button></div>
                        <div v-for="option in approveOptions" :key="option.value" class="menu-item" :class="{ warn: option.warn }" @click="pickApprove(option)">
                            <component :is="option.icon" class="item-icon" :size="16" />
                            <div class="item-main">
                                <div class="item-title">{{ option.label }}</div>
                                <div class="item-desc">{{ option.desc }}</div>
                                <Button v-if="option.value === 'auto'" variant="link" size="xs" class="config-link" @click.stop="goPermission">去配置</Button>
                            </div>
                            <Check v-if="approveMode === option.value" class="item-check" :size="14" />
                        </div>
                    </PopoverContent>
                </Popover>
                <Popover v-model:open="agentOpen">
                    <PopoverTrigger as-child>
                        <Button variant="ghost" class="tool-chip">
                            <span>{{ agentId ? '@ ' + agentName : '@ Agent' }}</span>
                            <ArrowUp class="chip-arrow" :class="{ open: agentOpen }" :size="14" />
                        </Button>
                    </PopoverTrigger>
                    <PopoverContent side="top" align="start" :side-offset="6" class="w-[220px]">
                        <div class="menu-label">内置智能体</div>
                        <div class="menu-item" @click="selectBuiltinAgent">
                            <span class="agent-avatar"><MessageSquare :size="12" /></span>
                            <div class="item-main"><div class="item-title">Agent</div></div>
                            <Check v-if="!agentId" class="item-check" :size="14" />
                        </div>
                        <div v-for="agent in customAgents" :key="agent.id" class="menu-item" @click="selectAgent(agent)">
                            <span class="agent-avatar"><User :size="12" /></span>
                            <div class="item-main"><div class="item-title">{{ agent.name }}</div></div>
                            <Check v-if="agentId === agent.id" class="item-check" :size="14" />
                        </div>
                        <Button class="menu-block-btn" size="sm" @click="onCreateAgent">创建智能体</Button>
                    </PopoverContent>
                </Popover>
            </div>
            <div class="toolbar-right">
                <Popover v-model:open="modelOpen">
                    <PopoverTrigger as-child>
                        <Button variant="ghost" class="tool-chip model-chip">
                            <span>{{ modelName || '未配置模型' }}</span>
                            <ArrowUp class="chip-arrow" :class="{ open: modelOpen }" :size="14" />
                        </Button>
                    </PopoverTrigger>
                    <PopoverContent side="top" align="end" :side-offset="6" class="w-[280px]">
                        <div class="menu-label">内置模型</div>
                        <div class="model-scroll">
                            <div v-if="!models.length" class="group-empty">暂无模型，请在 设置 → 模型 添加</div>
                            <Popover v-for="model in models" :key="`${model.provider}-${model.name}`" :open="paramKey === modelKey(model)">
                                <PopoverAnchor as-child>
                                    <div class="menu-item model-row" :class="{ disabled: modelBlocked(model) }" @click="pickModel(model)" @mouseenter="openParam(model)" @mouseleave="closeParam">
                                        <img v-if="providerIcon(model.provider)" class="model-avatar" :src="providerIcon(model.provider)" :alt="model.name">
                                        <Sparkles v-else class="item-icon" :size="15" />
                                        <span class="model-name-group">
                                            <span class="item-title">{{ model.name }}</span>
                                            <span v-if="model.vision" class="vision-badge">视觉</span>
                                        </span>
                                        <span class="item-meta">{{ model.display }} · {{ model.context }} · {{ thinkingLabel(model, model.thinking) }}</span>
                                        <Check v-if="modelName === model.name" class="item-check" :size="14" />
                                    </div>
                                </PopoverAnchor>
                                <PopoverContent side="right" align="start" :side-offset="8" class="w-auto min-w-[250px] max-w-[320px]">
                                    <div class="model-param" @click.stop @mouseenter="keepParam" @mouseleave="closeParam">
                                        <div class="param-title">模型参数 · {{ model.name }}</div>
                                        <div class="param-row">
                                            <span class="param-label">上下文</span>
                                            <RadioGroup class="flex flex-row flex-wrap items-center justify-end gap-2" :model-value="model.context" @update:model-value="(v) => setContext(model, v)">
                                                <span v-for="c in model.contextOptions" :key="c" class="param-option" @click="setContext(model, c)">
                                                    <RadioGroupItem :value="c" @click.stop />
                                                    <span>{{ c }}</span>
                                                </span>
                                            </RadioGroup>
                                        </div>
                                        <div class="param-row">
                                            <span class="param-label">{{ model.thinkingTitle }}</span>
                                            <RadioGroup class="flex flex-row flex-wrap items-center justify-end gap-2" :model-value="model.thinking" @update:model-value="(v) => setThinking(model, v)">
                                                <span v-for="t in model.thinkingOptions" :key="t.value" class="param-option" @click="setThinking(model, t.value)">
                                                    <RadioGroupItem :value="t.value" @click.stop />
                                                    <span>{{ t.label }}</span>
                                                </span>
                                            </RadioGroup>
                                        </div>
                                    </div>
                                </PopoverContent>
                            </Popover>
                        </div>
                        <Button class="menu-block-btn" size="sm" @click="onAddModel">添加模型</Button>
                    </PopoverContent>
                </Popover>
                <Button v-if="running" class="send-button stop-button" size="icon" aria-label="停止" title="停止本轮任务" @click="$emit('stop')">
                    <Square :size="14" fill="currentColor" />
                </Button>
                <Button v-else class="send-button" size="icon" :aria-label="canSend ? '发送' : '请先选择项目文件夹'" :title="canSend ? '发送' : '请先在顶栏选择项目文件夹'" :disabled="!canSend || !text.trim()" @click="send">
                    <ArrowUp :size="16" />
                </Button>
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ArrowUp, BookOpen, Check, CircleCheck, FileText, Folder, Info, Paperclip, Plus, Sparkles, Square, TriangleAlert, User, X, Zap, MessageSquare } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { Popover, PopoverAnchor, PopoverContent, PopoverTrigger } from '@/components/shadcn_ui/ui/popover'
import { RadioGroup, RadioGroupItem } from '@/components/shadcn_ui/ui/radio-group'
import { Textarea } from '@/components/shadcn_ui/ui/textarea'
import { toast } from '@/components/shadcn_ui/lib/useToast'
import InsertContext from '../../components/InsertContext.vue'
import { providerIcon } from '../../assets/providers'
import { deleteUpload, getPermission, listAgents, listModels, openDoc, projectFiles, savePermission, updateModel, uploadFile } from '../../api/request_data'
import { TRIGGER_MODES, applyCaretCoords, buildSendPayload, computeInsertText, computePathReplace, filterPathItems, isImageFile, matchPathTrigger, measureCaretCoords, modelKey, normalizeModelRow, resizeTextareaEl, resolveTextareaEl, revokeAttachmentUrls, thinkingLabelOf } from '../../utils/inputHelpers'
const props = defineProps({ draft: { type: String, default: '' }, canSend: { type: Boolean, default: true }, projectPath: { type: String, default: '' }, running: { type: Boolean, default: false } })
const emit = defineEmits(['send', 'stop', 'update:draft', 'open-settings', 'attachment', 'attachment-removed', 'model-change'])
const text = ref(props.draft)
const inputRef = ref(null)
const textareaEl = () => resolveTextareaEl(inputRef.value)
const resizeTextarea = () => resizeTextareaEl(textareaEl())
const focus = () => nextTick(() => { const el = textareaEl(); if (el) el.focus() })
defineExpose({ focus, reloadPermission: () => loadPermission(), currentModelPayload: () => ({ model: modelName.value, agent: agentId.value }) })
watch(() => props.draft, (value) => {
    text.value = value || ''
    nextTick(resizeTextarea)
})
// 草稿双向同步：展开/收起右栏或窗口尺寸变化会重建 splitter，中栏子组件随之重挂载，
// 只在清空时上报会让未发送的输入被清掉，所以这里改成全量同步
watch(text, (value) => { emit('update:draft', value) })
const onInput = () => {
    resizeTextarea()
    syncTrigger()
}
const insertAtCursor = (value) => {
    const el = textareaEl()
    const start = el && typeof el.selectionStart === 'number' ? el.selectionStart : text.value.length
    const end = el && typeof el.selectionEnd === 'number' ? el.selectionEnd : text.value.length
    const next = computeInsertText(text.value, start, end, value)
    text.value = next.text
    nextTick(() => {
        resizeTextarea()
        if (!el) return
        el.focus()
        el.setSelectionRange(next.pos, next.pos)
    })
}
const onAddFileToChat = (event) => {
    const detail = (event && event.detail) || {}
    const rel = String(detail.rel || '').trim()
    if (!rel) return
    insertAtCursor(`${detail.type === 'dir' ? '#Folder:' : '#File:'}${rel} `)
    focus()
}
const panelOpen = ref(false)
const panelMode = ref('context')
const panelTrigger = ref('')
const agentAnchorRef = ref(null)
const pathAnchorRef = ref(null)
const agentTriggerOpen = ref(false)
const openPanel = (mode, trigger) => {
    panelMode.value = mode
    panelTrigger.value = trigger
    panelOpen.value = true
}
const measureCaret = () => {
    const coords = measureCaretCoords(textareaEl(), agentAnchorRef.value, text.value)
    applyCaretCoords(coords, agentAnchorRef.value, pathAnchorRef.value)
}
const closeAgentTrigger = () => {
    agentTriggerOpen.value = false
}
const openAgentTrigger = () => {
    measureCaret()
    agentTriggerOpen.value = true
}
const pathOpen = ref(false)
const pathKind = ref('file')  // file | dir（对应 #File: / #Folder:）
const pathItems = ref([])
const pathLoading = ref(false)
const pathError = ref('')
let pathSeq = 0
let lastPathKey = ''
let pathStart = -1  // 触发前缀在输入框中的起始下标（失焦后仍可用）
const loadPathItems = async (kind, query) => {
    const key = `${kind}|${query}`
    if (key === lastPathKey) return
    lastPathKey = key
    const seq = ++pathSeq
    pathLoading.value = true
    pathError.value = ''
    try {
        const data = await projectFiles('', query || '')
        if (seq !== pathSeq) return
        pathItems.value = filterPathItems(data.list, kind, 30)
    } catch (err) {
        if (seq !== pathSeq) return
        pathItems.value = []
        pathError.value = `读取项目文件失败：${err.message}`
    } finally {
        if (seq === pathSeq) pathLoading.value = false
    }
}
const checkPathTrigger = () => {
    const el = textareaEl()
    const caret = el && typeof el.selectionStart === 'number' ? el.selectionStart : text.value.length
    const hit = matchPathTrigger(text.value, caret)
    if (!hit) {
        if (pathOpen.value) pathOpen.value = false
        return false
    }
    pathKind.value = hit.kind
    pathStart = hit.start
    measureCaret()
    pathOpen.value = true
    loadPathItems(hit.kind, hit.query)
    return true
}
const openPathPicker = (kind) => {
    pathKind.value = kind
    pathItems.value = []
    lastPathKey = ''
    const trigger = kind === 'dir' ? '#Folder:' : '#File:'
    pathStart = text.value.lastIndexOf(trigger)
    measureCaret()
    pathOpen.value = true
    loadPathItems(kind, '')
}
const pickPath = (item) => {
    const trigger = pathKind.value === 'dir' ? '#Folder:' : '#File:'
    const start = pathStart >= 0 ? pathStart : text.value.lastIndexOf(trigger)
    if (start < 0) {
        pathOpen.value = false
        return
    }
    const next = computePathReplace(text.value, start, trigger, item.rel)
    text.value = next.text
    pathOpen.value = false
    lastPathKey = ''
    pathStart = -1
    nextTick(() => {
        const el = textareaEl()
        if (!el) return
        el.focus()
        el.setSelectionRange(next.pos, next.pos)
    })
}
const syncTrigger = () => {
    const el = textareaEl()
    const caret = el && typeof el.selectionStart === 'number' ? el.selectionStart : text.value.length
    const char = text.value.slice(caret - 1, caret)
    if (agentTriggerOpen.value) {
        if (char === '@') return
        closeAgentTrigger()
    }
    if (panelOpen.value) {
        if (char === panelTrigger.value) return
        panelOpen.value = false
    }
    if (checkPathTrigger()) return  // #File:/#Folder: 优先于 # 上下文面板
    if (char === '@') {
        openAgentTrigger()
        return
    }
    const mode = TRIGGER_MODES[char]
    if (mode) openPanel(mode, char)
}
const onPanelSelect = (payload) => {
    panelOpen.value = false
    if (!payload) return
    if (payload.kind === 'command') insertAtCursor(`${payload.item.value} `)
    else if (payload.kind === 'skill') insertAtCursor(`${payload.item.name} `)
    else if (payload.kind === 'context' && payload.item && (payload.item.value === 'file' || payload.item.value === 'folder')) {
        insertAtCursor(payload.item.value === 'file' ? '#File:' : '#Folder:')
        nextTick(() => openPathPicker(payload.item.value === 'file' ? 'file' : 'dir'))
    }
}
const selectTriggerAgent = () => {
    agentId.value = ''
    agentName.value = 'Agent'
    closeAgentTrigger()
    const el = textareaEl()
    const caret = el && typeof el.selectionStart === 'number' ? el.selectionStart : text.value.length
    if (text.value.slice(caret - 1, caret) !== '@') {
        if (el) el.focus()
        return
    }
    text.value = `${text.value.slice(0, caret - 1)}${text.value.slice(caret)}`
    nextTick(() => {
        if (!el) return
        el.focus()
        el.setSelectionRange(caret - 1, caret - 1)
    })
}
const onCreateAgent = () => {
    agentOpen.value = false
    closeAgentTrigger()
    emit('open-settings', { tab: 'agents', createAgent: true })
}
const selectAgent = (agent) => {
    agentId.value = agent.id
    agentName.value = agent.name
    agentOpen.value = false
    closeAgentTrigger()
}
const selectBuiltinAgent = () => {
    agentId.value = ''
    agentName.value = 'Agent'
    agentOpen.value = false
}
const onManageSkills = () => emit('open-settings', { tab: 'skills' })
const onAgentDocMousedown = (event) => {
    const target = event.target
    if (target instanceof Element && target.closest('.agent-menu')) return
    closeAgentTrigger()
}
const onAgentDocKeydown = (event) => {
    if (event.key === 'Escape') closeAgentTrigger()
}
const unbindAgentTrigger = () => {
    document.removeEventListener('mousedown', onAgentDocMousedown)
    window.removeEventListener('keydown', onAgentDocKeydown)
}
watch(agentTriggerOpen, (open) => {
    if (open) {
        document.addEventListener('mousedown', onAgentDocMousedown)
        window.addEventListener('keydown', onAgentDocKeydown)
    } else {
        unbindAgentTrigger()
    }
})
onBeforeUnmount(() => {
    unbindAgentTrigger()
    revokeAttachmentUrls(attachments.value)
    clearTimeout(paramTimer)
    window.removeEventListener('lwa:add-file-to-chat', onAddFileToChat)
    window.removeEventListener('lwa:models-changed', loadModels)
})
const plusOpen = ref(false)
const fileRef = ref(null)
const uploading = ref(false)
const onAddContext = () => {
    plusOpen.value = false
    insertAtCursor('#')
    nextTick(() => openPanel('context', '#'))
}
const onPickSlash = () => {
    plusOpen.value = false
    insertAtCursor('/')
    nextTick(() => openPanel('slash', '/'))
}
const onUploadClick = () => {
    plusOpen.value = false
    nextTick(() => fileRef.value && fileRef.value.click())
}
const attachments = ref([])
const onFileChange = async (event) => {
    const files = Array.from((event.target && event.target.files) || [])
    if (!files.length) return
    uploading.value = true
    try {
        for (const file of files) {
            const isImage = isImageFile(file)
            if (isImage && !currentVision.value) {  // 非多模态模型禁止上传图片
                toast.warning('模型不支持多模态')
                continue
            }
            try {
                const data = await uploadFile(file)
                const item = { name: data.name, path: data.path, isImage, size: Number(data.size || file.size || 0) }
                if (isImage) item.url = URL.createObjectURL(file)
                attachments.value.push(item)
                emit('attachment', { name: item.name, path: item.path })
            } catch (err) {
                toast.error(`上传失败：${err.message}`)
            }
        }
    } finally {
        uploading.value = false
        event.target.value = ''
    }
}
const removeAttachment = async (index) => {
    const item = attachments.value[index]
    if (!item) return
    try {
        await deleteUpload(item.path)
    } catch (err) {
        toast.error(`移除附件失败：${err.message}`)
        return
    }
    revokeAttachmentUrls([item])
    attachments.value.splice(index, 1)
    emit('attachment-removed', { name: item.name, path: item.path })
}
const approveOpen = ref(false)
const agentOpen = ref(false)
const modelOpen = ref(false)
const agentName = ref('Agent')
const agentId = ref('')
const customAgents = ref([])
const approveMode = ref('auto')
const approveOptions = [
    { value: 'manual', label: '手动审批', desc: '重要操作由你确认', icon: CircleCheck },
    { value: 'auto', label: '自动审批', desc: '由 AI 审核，必要时再询问你', icon: TriangleAlert },
    { value: 'full', label: '完全访问', desc: '不经审批，直接在本机运行', icon: TriangleAlert, warn: true },
]
const approveText = computed(() => (approveOptions.find((option) => option.value === approveMode.value) || approveOptions[0]).label)
const persistApproveMode = async (mode) => {
    const data = await getPermission()
    await savePermission({ ...data, mode })
}
const pickApprove = async (option) => {
    const previous = approveMode.value
    approveMode.value = option.value
    try {
        await persistApproveMode(option.value)
    } catch (err) {
        approveMode.value = previous
        toast.error(`保存审批模式失败：${err.message}`)
    }
}
const goPermission = async () => {
    try {
        await persistApproveMode(approveMode.value)
    } catch (err) {
        toast.error(`保存审批模式失败：${err.message}`)
        return
    }
    approveOpen.value = false
    emit('open-settings', { tab: 'permission', permissionMode: approveMode.value })
}
const openPermissionDoc = () => {
    openDoc('permission.md').catch((err) => toast.error(`打开文档失败：${err.message}`))
}
const loadPermission = async () => {
    try {
        const data = await getPermission()
        if (['manual', 'auto', 'full'].includes(data.mode)) approveMode.value = data.mode
    } catch {
        /* 读取失败保持默认 */
    }
}
const loadAgents = async () => {
    try {
        const data = await listAgents()
        customAgents.value = (data.list || []).filter((item) => item.enabled)
    } catch {
        customAgents.value = []
    }
}
const models = ref([])
const modelName = ref('')
const providerName = ref('')
const paramKey = ref('')
let paramTimer = 0
const thinkingLabel = thinkingLabelOf
const emitCurrentModel = () => {
    const hit = models.value.find((model) => model.name === modelName.value && (!providerName.value || model.provider === providerName.value)) || models.value.find((model) => model.name === modelName.value) || null
    emit('model-change', hit ? { provider: hit.provider, model: hit.name, context: hit.context } : { provider: '', model: modelName.value, context: '' })
}
const openParam = (model) => {
    clearTimeout(paramTimer)
    paramKey.value = modelBlocked(model) ? '' : modelKey(model)
}
const keepParam = () => clearTimeout(paramTimer)
const closeParam = () => {
    clearTimeout(paramTimer)
    paramTimer = window.setTimeout(() => { paramKey.value = '' }, 120)
}
const currentVision = computed(() => {
    const hit = models.value.find((model) => model.name === modelName.value && (!providerName.value || model.provider === providerName.value)) || models.value.find((model) => model.name === modelName.value)
    return Boolean(hit && hit.vision)
})
const hasImageAttachment = computed(() => attachments.value.some((item) => item.isImage))
const modelBlocked = (model) => Boolean(model.disabled || (hasImageAttachment.value && !model.vision))
const loadModels = async () => {
    try {
        const data = await listModels()
        models.value = (data.list || []).map((item, index) => normalizeModelRow(item, index))
        const matched = models.value.find((model) => model.name === modelName.value && (!providerName.value || model.provider === providerName.value) && !model.disabled)
        const first = matched || models.value.find((model) => model.name === modelName.value && !model.disabled) || models.value.find((model) => !model.disabled)
        modelName.value = first ? first.name : ''
        providerName.value = first ? first.provider : ''
        emitCurrentModel()
    } catch (err) {
        models.value = []
        toast.error(`读取模型失败：${err.message}`)
    }
}
const saveParam = async (model, patch) => {
    try {
        const res = await updateModel(model.index, patch)
        const updated = Array.isArray(res && res.list) && res.list[model.index] ? normalizeModelRow(res.list[model.index], model.index) : null
        Object.assign(model, updated || patch)
        if (model.name === modelName.value && (!providerName.value || model.provider === providerName.value)) emitCurrentModel()
        window.dispatchEvent(new Event('lwa:model-param-saved'))
        toast.success('模型参数已保存')
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    }
}
const setContext = (model, v) => saveParam(model, { context: v })
const setThinking = (model, v) => saveParam(model, { thinking: v })
const pickModel = (model) => {
    if (modelBlocked(model)) return
    modelName.value = model.name
    providerName.value = model.provider
    emitCurrentModel()
    paramKey.value = ''
    modelOpen.value = false
}
const onAddModel = () => {
    modelOpen.value = false
    emit('open-settings', { tab: 'model', addModel: true })
}
const send = () => {
    if (props.running) return
    const content = text.value.trim()
    if (!content) return
    closeAgentTrigger()
    panelOpen.value = false
    const payload = buildSendPayload(attachments.value)
    emit('send', { content, model: modelName.value, agent: agentId.value, attachments: payload })
    revokeAttachmentUrls(attachments.value)
    attachments.value = []
    text.value = ''
    emit('update:draft', '')
    nextTick(resizeTextarea)
}
const onKeydown = (event) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
        event.preventDefault()
        send()
    }
}
onMounted(() => {
    loadModels()
    loadPermission()
    loadAgents()
    resizeTextarea()
    window.addEventListener('lwa:add-file-to-chat', onAddFileToChat)
    window.addEventListener('lwa:models-changed', loadModels)
})
</script>

<style scoped>
.input-message { position: relative; padding: 10px 12px 8px; border: 1px solid var(--border); border-radius: 10px; background: var(--card); }
.agent-anchor { position: absolute; width: 0; height: 0; pointer-events: none; }
.input-textarea { padding: 6px 4px 10px; border: none; background: transparent; box-shadow: none; font-size: 13px; line-height: 1.7; min-height: 0; resize: none; overflow-y: auto; }
.input-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 6px; min-width: 0; }
.tool-chip { gap: 4px; height: 26px; padding: 0 6px; border: none; border-radius: 0; background: transparent; font-size: 12px; color: var(--foreground); }
.tool-chip:hover { background: var(--accent); }
.model-chip { max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; gap: 6px; }
.model-chip .chip-arrow { margin-left: 8px; }
.chip-arrow { transition: transform 0.15s ease; }
.chip-arrow.open { transform: rotate(180deg); }
.menu-head { display: flex; align-items: center; justify-content: space-between; padding: 4px 0 8px; font-size: 12px; color: var(--muted-foreground); }
.menu-label { padding: 4px 0 6px; font-size: 12px; color: var(--muted-foreground); }
.menu-item { display: flex; align-items: center; gap: 8px; padding: 7px 8px; border-radius: 6px; cursor: pointer; }
.menu-item:hover { background: var(--accent); }
.menu-item.disabled { opacity: 0.45; cursor: not-allowed; }
.plus-menu { display: flex; flex-direction: column; gap: 2px; }
.menu-divider { height: 1px; margin: 4px 6px; background: var(--border); }
.item-icon { flex: none; color: var(--foreground); }
.model-avatar { width: 16px; height: 16px; flex: none; border-radius: 4px; object-fit: contain; }
.item-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.item-title { font-size: 13px; color: var(--foreground); }
.item-desc { font-size: 11px; color: var(--muted-foreground); }
.config-link { align-self: flex-start; height: auto; font-size: 11px; }
.item-check { flex: none; color: #22c55e; }
.menu-item.warn .item-title { color: #f59e0b; }
.menu-item.warn .item-desc { color: #f59e0b; }
.menu-item.warn .item-icon { color: #f59e0b; }
.agent-avatar { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; flex: none; border-radius: 6px; color: #22c55e; background: color-mix(in srgb, #22c55e 12%, transparent); }
.menu-block-btn { width: 100%; margin-top: 6px; }
.model-scroll { max-height: 300px; overflow-y: auto; }
.group-empty { padding: 10px 8px; font-size: 12px; color: var(--muted-foreground); }
.model-row { display: grid; grid-template-columns: 16px 112px minmax(0, 1fr) 14px; align-items: center; gap: 8px; }
.model-name-group { display: flex; align-items: center; gap: 6px; min-width: 0; }
.model-row .item-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.model-row .item-meta { min-width: 0; overflow: hidden; font-size: 11px; color: var(--muted-foreground); text-overflow: ellipsis; white-space: nowrap; }
.model-row .item-check { justify-self: end; }
.vision-badge { flex: none; padding: 0 5px; border-radius: 8px; font-size: 10px; line-height: 15px; color: #22c55e; background: color-mix(in srgb, #22c55e 12%, transparent); }
.model-param { display: flex; flex-direction: column; gap: 10px; padding: 2px 0; }
.param-title { font-size: 12px; color: var(--muted-foreground); }
.param-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.param-label { flex: none; font-size: 12px; color: var(--foreground); }
.param-option { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: var(--foreground); cursor: pointer; }
.file-input { display: none; }
.path-menu { display: flex; flex-direction: column; gap: 2px; max-height: 360px; overflow-y: auto; }
.path-name { flex: none; max-width: 45%; overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.path-abs { flex: 1; min-width: 0; overflow: hidden; font-size: 11px; text-align: right; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.send-button { flex: none; border-radius: 9999px; }
.stop-button { background: var(--destructive); color: #fff; }
.stop-button:hover { background: color-mix(in srgb, var(--destructive) 85%, #000); }
.attach-bar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 2px 2px 8px; }
.attach-item { position: relative; display: flex; align-items: center; gap: 6px; max-width: 180px; height: 26px; padding: 0 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--muted); }
.attach-item.is-image { width: 56px; height: 56px; max-width: none; padding: 0; border-radius: 6px; overflow: visible; }
.attach-thumb { display: block; width: 100%; height: 100%; border-radius: 6px; object-fit: cover; border: 1px solid var(--border); }
.attach-file-icon { flex: none; color: var(--muted-foreground); }
.attach-name { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.attach-remove { position: absolute; top: -6px; right: -6px; display: inline-flex; align-items: center; justify-content: center; width: 16px; height: 16px; padding: 0; border: none; border-radius: 50%; background: var(--destructive); color: #fff; cursor: pointer; opacity: 0; transition: opacity 0.15s ease; }
.attach-item:hover .attach-remove { opacity: 1; }
.attach-remove:hover { filter: brightness(1.1); }
</style>
