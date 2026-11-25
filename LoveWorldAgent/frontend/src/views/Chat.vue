<template>
    <div class="chat-page">
        <div ref="splitterEl" class="chat-splitter">
            <aside class="chat-side-panel" :style="{ width: `${sideWidth}px` }">
                <div class="chat-side">
                    <div class="side-top">
                        <Button variant="outline" class="new-chat" @click="onNewSession">
                            <Plus class="size-4" />
                            <span>新建会话</span>
                        </Button>
                    </div>
                    <SessionList :sessions="sessions" :active-id="activeId" :keyword="keyword" :loading="sessionsLoading" @select="onSelectSession" @rename="onRenameSession" @delete="onDeleteSession" />
                    <footer class="side-foot">
                        <SelfPopover @settings="setingOpen = true" />
                        <Button variant="ghost" size="icon-sm" class="foot-gear" aria-label="设置" @click="setingOpen = true"><Settings class="size-4" /></Button>
                    </footer>
                </div>
            </aside>
            <div class="chat-bar" role="separator" aria-orientation="vertical" aria-label="调整会话列表宽度" title="拖动调整宽度，双击复位" tabindex="0" @pointerdown="startDrag(0, $event)" @dblclick="resetSideWidth" @keydown="onBarKeydown(0, $event)"></div>
            <section class="chat-main-panel" :style="{ minWidth: `${MAIN_MIN}px` }">
                <div class="chat-main">
                    <header class="chat-topbar">
                        <button class="project-switch" :class="{ missing: !projectPath }" :title="projectPath || '选择智能体工作目录'" @click="pickProject">
                            <FolderOpen class="size-4" />
                            <span>{{ projectName || '请选择文件夹' }}</span>
                        </button>
                        <button class="panel-toggle" :aria-label="rightOpen ? '关闭右侧面板' : '打开右侧面板'" @click="rightOpen = !rightOpen">
                            <component :is="rightOpen ? PanelRightClose : PanelRightOpen" class="size-4" />
                        </button>
                    </header>
                    <MessageListFather ref="listRef" :turns="turns" :rollback-id="rollbackTurnKey" @recall="onRecall" @delete="onDeleteTurn" @retry="onRetryTurn" @rollback-confirm="onRollbackConfirm" @rollback-cancel="onRollbackCancel" @decision="onDecision" @answer="onAnswer" @open-file="onOpenFile" @continue="onContinueTurn" />
                    <InputMessage ref="inputRef" :draft="pendingDraft" :can-send="Boolean(projectPath)" :project-path="projectPath" :running="isRunning" @send="onSend" @stop="onStop" @update:draft="pendingDraft = $event" @open-settings="onOpenSettings" @attachment="onAttachment" @attachment-removed="onAttachmentRemoved" @model-change="onModelChange" />
                </div>
            </section>
            <div v-if="rightOpen" class="chat-bar" role="separator" aria-orientation="vertical" aria-label="调整右侧面板宽度" title="拖动调整宽度，双击复位" tabindex="0" @pointerdown="startDrag(1, $event)" @dblclick="resetRightWidth" @keydown="onBarKeydown(1, $event)"></div>
            <aside v-if="rightOpen" class="chat-right-panel" :style="{ width: `${rightWidth}px` }">
                <RightTools ref="rightToolsRef" v-model:open="rightOpen" v-model:active-tab="rightTab" :project-path="projectPath" :built-tools="builtTools" :todos="todos" :todo-enabled="chatPrefs.todo" :rules="rules" :skills="skills" :context-files="contextFiles" :context-others="contextOthers" :context-tokens="contextTokens" :context-limit="contextLimit" :usage-detail="usageDetail" :running="isRunning" :compressing="isCompressing" @compress="onCompress" />
            </aside>
        </div>
        <SetingDialog v-model:visible="setingOpen" :open-state="setingState" @permission-saved="onPermissionSaved" @open-browser="onBrowserSettingsChanged" />
        <ClearData v-model:open="clearDataOpen" @cleared="onDataCleared" />
        <Dialog v-model:open="renameOpen">
            <DialogContent class="max-w-sm">
                <DialogHeader>
                    <DialogTitle>重命名会话</DialogTitle>
                    <DialogDescription class="sr-only">输入新的会话标题</DialogDescription>
                </DialogHeader>
                <Input v-model="renameText" maxlength="60" placeholder="请输入新的会话标题" @keyup.enter="onRenameConfirm" />
                <DialogFooter>
                    <Button variant="outline" size="sm" @click="renameOpen = false">取消</Button>
                    <Button size="sm" :disabled="renameSaving" @click="onRenameConfirm">保存</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { FolderOpen, PanelRightClose, PanelRightOpen, Plus, Settings } from 'lucide-vue-next'
import { Button } from '../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../components/shadcn_ui/ui/dialog'
import { Input } from '../components/shadcn_ui/ui/input'
import { toast } from '../components/shadcn_ui/lib/useToast'
import { confirm } from '../components/shadcn_ui/lib/useConfirm'
import { approveChat, chatSoundUrl, compressContext, createSession, deleteSession, deleteTurnMessages, getBrowserSettings, getChatSse, getComputerUse, getExcelOption, getLayout, getMcpStatus, getMobileOption, getPptOption, getProject, getSessionContext, getSessionUsage, getWordOption, listMessages, listModels, listRelus, listSessions, listSkills, listTerminalSessions, saveLayout, setProject, stopChat, streamChat, updateSession } from '../api/request_data'
import { getNumber, setNumber } from '../api/local_storage'
import InputMessage from './chats/InputMessage.vue'
import MessageListFather from './chats/MessageListFather.vue'
import SessionList from './chats/SessionList.vue'
import ClearData from './others/ClearData.vue'
import SelfPopover from '../components/SelfPopover.vue'
import RightTools from './RightTools.vue'
import SetingDialog from './SetingDialog.vue'
import { appendUserMessage, applyHistory, createInitialState, groupTurns, markActive, markDecision, markExpired, pendingActions, reduceAgentEvent, removeTurnMessages } from '../utils/chatEventReducer'
import { blocksOf } from '../utils/chatContract'
import { findToolRequest, latestTodos, toolResponseOutput } from '../components/md_show/goosePart'
import { API_BASE, MAIN_MIN, NOTIFY_TEXT, PAGE_PADDING, RIGHT_KEY, RIGHT_MIN, RUNNING_STATUS, SIDE_DEFAULT, SIDE_KEY, SIDE_MIN, baseName, buildChatHistory, calcDefaultRightWidth, calcMaxRightWidth, calcMaxSideWidth, clampRight, clampSide, computeBaselineContext, extractContextRefs, extractTerminalTarget, hostName, mergeUsageDetail, messageUsageDetail, normalizeSavedTab, nowTime, parseContextSize, parseJson, parseTodoRows, pickTerminalSession, providerUsageDetail, relOfPath, resolveStaleSessionTarget } from '../utils/chatHelpers'
const sessionState = ref(createInitialState())
const turns = computed(() => groupTurns(sessionState.value))
const pendingDraft = ref('')
const rollbackTurnKey = ref('')
const sessions = ref([])
const activeId = ref('')
const keyword = ref('')
const sessionsLoading = ref(false)
const setingOpen = ref(false)
const setingState = ref({})
const onOpenSettings = (state) => {
    setingState.value = state || {}
    setingOpen.value = true
}
const inputRef = ref(null)
const listRef = ref(null)
const onPermissionSaved = () => {
    if (inputRef.value) inputRef.value.reloadPermission()
}
const builtTools = ref({ browser: true, computer: false, word: false, ppt: false, excel: false, mobile: false })  // 内置工具各模块启用态
const sideWidth = ref(264)
const rightOpen = ref(false)
const rightWidth = ref(420)
const rightTab = ref('')  // 右栏页签提升到 Chat 持有：splitter 重建 / 右栏 v-if 重挂载都不再丢失
const loadBuiltTools = async () => {
    const [br, cu, wd, pp, ex, mb] = await Promise.all([
        getBrowserSettings().catch(() => null),
        getComputerUse().catch(() => null),
        getWordOption().catch(() => null),
        getPptOption().catch(() => null),
        getExcelOption().catch(() => null),
        getMobileOption().catch(() => null),
    ])
    const next = {
        browser: br ? br.enabled !== false : builtTools.value.browser,
        computer: cu ? Boolean(cu.enabled) : builtTools.value.computer,
        word: wd ? Boolean(wd.enabled) : builtTools.value.word,
        ppt: pp ? Boolean(pp.enabled) : builtTools.value.ppt,
        excel: ex ? Boolean(ex.enabled) : builtTools.value.excel,
        mobile: mb ? Boolean(mb.enabled) : builtTools.value.mobile,
    }
    builtTools.value = next
}
const onBrowserSettingsChanged = () => { loadBuiltTools() }
// 窗口布局记忆：写入 config.json 的 ui.layout，默认开启；拖动结束 / 开关右栏 / 切页签后防抖 500ms 落盘
const layoutRemember = ref(true)
const layoutReady = ref(false)
let layoutSaveTimer = null
const layoutPayload = () => ({ remember: Boolean(layoutRemember.value), sideWidth: sideWidth.value, rightWidth: rightWidth.value, rightOpen: Boolean(rightOpen.value), rightTab: rightTab.value })
const scheduleLayoutSave = () => {
    if (!layoutReady.value || !layoutRemember.value) return
    clearTimeout(layoutSaveTimer)
    layoutSaveTimer = setTimeout(() => { saveLayout(layoutPayload()).catch(() => {}) }, 500)
}
const onLayoutRemember = (event) => {
    layoutRemember.value = Boolean(event.detail && event.detail.remember)
    if (layoutRemember.value) saveLayout(layoutPayload()).catch(() => {})
}
const restoreLayout = async () => {
    try {
        const data = await getLayout()
        if (!data || data.remember === false) {
            layoutRemember.value = false
            layoutReady.value = true
            return
        }
        layoutRemember.value = true
        if (Number.isFinite(data.sideWidth) && data.sideWidth > 0) sideWidth.value = clampSide(data.sideWidth)
        if (Number.isFinite(data.rightWidth) && data.rightWidth > 0) {
            rightWidth.value = clampRight(data.rightWidth)
            rightFollowRatio.value = false
        }
        rightOpen.value = data.rightOpen === true
        rightTab.value = normalizeSavedTab(data.rightTab)
    } catch { /* 后端未就绪时沿用本地存储值 */ }
    layoutReady.value = true
}
const defaultRightWidth = () => calcDefaultRightWidth(window.innerWidth)
const rightFollowRatio = ref(false)
let rightResizeTimer = null
// 自研拖拽分隔条：容器宽度只受三栏 + 两条分隔条约束，主区始终保留 MAIN_MIN
const splitterEl = ref(null)
const containerWidth = () => {
    const rect = splitterEl.value && splitterEl.value.getBoundingClientRect()
    return rect && rect.width ? rect.width : Math.max(0, window.innerWidth - PAGE_PADDING)
}
const maxSideWidth = () => calcMaxSideWidth(containerWidth(), rightOpen.value, rightWidth.value)
const maxRightWidth = () => calcMaxRightWidth(containerWidth(), rightOpen.value, sideWidth.value)
const fitWidths = () => {
    if (rightOpen.value) rightWidth.value = clampRight(Math.min(rightWidth.value, maxRightWidth()))
    sideWidth.value = clampSide(Math.min(sideWidth.value, maxSideWidth()))
}
const onWindowResize = () => {
    clearTimeout(rightResizeTimer)
    rightResizeTimer = setTimeout(() => {
        if (rightFollowRatio.value) rightWidth.value = defaultRightWidth()
        fitWidths()
        scheduleLayoutSave()
    }, 200)
}
// 拖动结束才落盘：拖动过程只改响应式宽度，避免每帧写 localStorage 与后端
const commitWidth = (index) => {
    if (index === 0) {
        sideWidth.value = clampSide(sideWidth.value)
        setNumber(SIDE_KEY, sideWidth.value)
        return
    }
    rightWidth.value = clampRight(rightWidth.value)
    rightFollowRatio.value = false
    setNumber(RIGHT_KEY, rightWidth.value)
}
let stopDrag = null
const startDrag = (index, event) => {
    if (stopDrag) return
    const startX = event.clientX
    const startSide = sideWidth.value
    const startRight = rightWidth.value
    event.preventDefault()
    const move = (moveEvent) => {
        const delta = moveEvent.clientX - startX
        if (index === 0) sideWidth.value = Math.max(SIDE_MIN, Math.min(maxSideWidth(), startSide + delta))
        else rightWidth.value = Math.max(RIGHT_MIN, Math.min(maxRightWidth(), startRight - delta))
    }
    const up = () => {
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', up)
        document.body.style.userSelect = ''
        document.body.style.cursor = ''
        stopDrag = null
        commitWidth(index)
    }
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    stopDrag = up
}
const onBarKeydown = (index, event) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    event.preventDefault()
    const step = event.shiftKey ? 24 : 8
    const delta = event.key === 'ArrowLeft' ? -step : step
    if (index === 0) sideWidth.value = Math.max(SIDE_MIN, Math.min(maxSideWidth(), sideWidth.value + delta))
    else rightWidth.value = Math.max(RIGHT_MIN, Math.min(maxRightWidth(), rightWidth.value - delta))
    commitWidth(index)
}
const resetSideWidth = () => {
    sideWidth.value = clampSide(SIDE_DEFAULT)
    setNumber(SIDE_KEY, sideWidth.value)
}
const resetRightWidth = () => {
    rightWidth.value = clampRight(defaultRightWidth())
    rightFollowRatio.value = true
    setNumber(RIGHT_KEY, rightWidth.value)
}
const projectPath = ref('')
const projectName = computed(() => baseName(projectPath.value))
const rightToolsRef = ref(null)
const todos = ref([])
const skills = ref([])
const rules = ref([])
const skillCatalog = ref([])
const ruleCatalog = ref([])
const mcpServersActive = ref([])
const sessionContextData = ref({ summary: null, facts: [] })
const contextFiles = ref([])
const contextOthers = ref([])
const contextTokens = ref(0)
const contextLimit = ref(200000)
const usageDetail = ref({})
const chatPrefs = ref({ ask: true, todo: true, notify: { banner: true, sound: true, volume: 100 } })
const currentModel = ref('')
const currentProvider = ref('')
const isCompressing = ref(false)
const addContextFile = (path) => {
    const rel = relOfPath(path, projectPath.value)
    if (!rel) return ''
    if (!contextFiles.value.includes(rel)) contextFiles.value = [...contextFiles.value, rel].slice(-30)
    return rel
}
const addContextRule = (name) => {
    const key = String(name || '').trim()
    if (!key || rules.value.some((item) => item.name === key)) return
    const hit = ruleCatalog.value.find((item) => item.name === key) || {}
    rules.value = [...rules.value, { name: key, trigger: hit.trigger || '', preview: hit.preview || '' }].slice(-30)
}
const addContextSkill = (name) => {
    const key = String(name || '').trim()
    if (!key || skills.value.some((item) => item.name === key)) return
    const hit = skillCatalog.value.find((item) => item.name === key) || {}
    skills.value = [...skills.value, { name: key, description: hit.description || '' }].slice(-30)
}
const syncBaselineContext = (messages = sessionState.value.messages || []) => {
    const next = computeBaselineContext({
        messages,
        ruleCatalog: ruleCatalog.value,
        skillCatalog: skillCatalog.value,
        sessionContextData: sessionContextData.value,
        mcpServersActive: mcpServersActive.value,
        projectPath: projectPath.value,
        prevOthers: contextOthers.value,
    })
    rules.value = next.rules
    skills.value = next.skills
    contextFiles.value = next.contextFiles
    contextOthers.value = next.contextOthers
}
const notify = (kind) => {
    const text = NOTIFY_TEXT[kind] || NOTIFY_TEXT.win
    const prefs = chatPrefs.value.notify || {}
    if (prefs.sound !== false) {
        try {
            const audio = new Audio(chatSoundUrl(kind))
            audio.volume = Math.min(1, Math.max(0, Number(prefs.volume ?? 100) / 100))
            audio.play().catch(() => {})
        } catch { /* 提示音播放失败静默 */ }
    }
    if (prefs.banner === false) return
    // 原生通知改走后端接口（桌面端由 Python 侧发 Windows toast），失败静默
    try {
        fetch(`${API_BASE}/api/app/notify`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ title: text.title, body: text.body }) }).catch(() => {})
    } catch { /* 原生通知失败静默 */ }
}
const withPanel = (fn, attempt = 0) => {
    rightOpen.value = true
    nextTick(() => {
        if (rightToolsRef.value) { fn(rightToolsRef.value); return }
        if (attempt < 12) setTimeout(() => withPanel(fn, attempt + 1), 40)
    })
}
const addContextOther = (label, kind, path = '') => {
    const name = String(label || '').trim()
    if (!name) return
    const rest = contextOthers.value.filter((item) => item.label !== name)
    contextOthers.value = [{ label: name, kind, path: String(path || '') }, ...rest].slice(0, 30)
}
let terminalAttachSeq = 0
const attachAgentTerminal = async (panel, argsObj, metaObj, resultText) => {
    const { directSid, directCwd } = extractTerminalTarget(argsObj, metaObj, resultText, projectPath.value)
    if (directSid) {
        panel.attach(directSid, directCwd)
        return
    }
    const seq = ++terminalAttachSeq
    const tryAttachLatest = async () => {
        const res = await listTerminalSessions('agent').catch(() => null)
        if (seq !== terminalAttachSeq) return false
        const pick = pickTerminalSession(res?.list, directCwd)
        if (pick && pick.id) {
            panel.attach(pick.id, String(pick.cwd || directCwd || ''))
            return true
        }
        return false
    }
    if (!(await tryAttachLatest())) {
        setTimeout(() => { if (seq === terminalAttachSeq) tryAttachLatest() }, 300)
    }
}
const syncPanelForTool = (name, args, result, meta = null) => {
    const tool = String(name || '')
    const params = parseJson(args)
    const data = parseJson(result)
    if (tool.startsWith('browser_')) {
        withPanel((panel) => panel.openBuild('browser'))
        if (tool === 'browser_take_screenshot' && result) {
            const shot = parseJson(result)
            const path = String((shot.data && shot.data.path) || shot.path || '')
            if (path) addContextOther(baseName(path), '浏览器截图', path)
        } else if ((tool === 'browser_navigate' || tool === 'browser_open') && params.url) {
            addContextOther(hostName(params.url), '联网页面', params.url)
        }
        return
    }
    if (tool.startsWith('computer_')) {
        withPanel((panel) => panel.openBuild('computer'))
        return
    }
    if (tool.startsWith('mobile_')) {
        withPanel((panel) => panel.openBuild('mobile'))
        return
    }
    if (tool.startsWith('file_') || tool.startsWith('rule_') || tool.startsWith('skill_')) {
        if (tool === 'rule_read') addContextRule(params.name || data.name)
        else if (tool === 'skill_read') addContextSkill(params.name || data.name)
        else if (tool === 'file_read' || tool === 'file_write') addContextFile(params.path || data.path)
        withPanel((panel) => {
            panel.openPanel('file')
            if (tool !== 'file_read' && tool !== 'file_write') return
            const rel = addContextFile(params.path || data.path)
            if (rel) panel.openFile(rel)
        })
        return
    }
    if (tool.startsWith('word_') || tool.startsWith('ppt_') || tool.startsWith('excel_')) {
        withPanel((panel) => panel.openBuild('office'))
        if (!tool.startsWith('word_')) return
        const ok = data && data.ok !== false && !(data.error)
        const p = String((data && data.data && data.data.path) || params.path || '')
        if (p) addContextOther(baseName(p), 'Word 文档', p)
        if (!ok || !p) return
        const writeTools = ['word_document', 'word_write', 'word_table', 'word_format', 'word_structure', 'word_media', 'word_batch']
        if (tool === 'word_document' && !['create', 'open', 'save_as', 'export_pdf'].includes(String(params.action || ''))) return
        if (writeTools.includes(tool)) {
            const rel = relOfPath(p, projectPath.value)
            if (rel) withPanel((panel) => { panel.openPanel('file'); panel.openFile(rel, true) })
        }
        return
    }
    if (tool === 'command_run' || tool.startsWith('terminal_')) {
        withPanel((panel) => {
            panel.openPanel('terminal')
            if (tool === 'terminal_close') return
            const metaObj = meta && typeof meta === 'object' ? meta : data
            attachAgentTerminal(panel, params, metaObj, result)
        })
        return
    }
    if (tool === 'todo_write') withPanel((panel) => panel.openPanel('todo'))
}
const modelList = ref([])
const applyModelLimit = (name, provider = currentProvider.value) => {
    const hit = (provider && modelList.value.find((item) => item.provider === provider && item.model === name)) || modelList.value.find((item) => item.model === name)
    const size = hit ? parseContextSize(hit.context) : 0
    if (size) contextLimit.value = size
}
const onModelChange = (payload = {}) => {
    if (payload.provider !== undefined) currentProvider.value = String(payload.provider || '')
    if (payload.model !== undefined) currentModel.value = String(payload.model || '')
    const size = payload.context ? parseContextSize(payload.context) : 0
    if (size) contextLimit.value = size
    else if (currentModel.value) applyModelLimit(currentModel.value, currentProvider.value)
}
/** 历史/压缩回传：只取用户提问与助手正文；会话本身由后端落库维护 */
const buildHistory = (excludeTurnKey = '') => buildChatHistory(sessionState.value.messages, excludeTurnKey)
const turnOf = (turnKey) => turns.value.find((item) => item.turnKey === turnKey) || null
const notifiedActions = new Set()
const applyUsage = (detail, accumulate = false) => {
    if (!detail) return
    const next = mergeUsageDetail(usageDetail.value, detail, accumulate)
    usageDetail.value = next
    contextTokens.value = Number(next.total || 0)
}
const syncToolResponse = (block) => {
    const info = findToolRequest(sessionState.value.messages, block.id)
    if (!info) return
    const output = toolResponseOutput(block)
    if (info.name === 'todo_write') {
        const rows = parseTodoRows(output)
        if (rows.length) todos.value = rows
        withPanel((panel) => panel.openPanel('todo'))
        return
    }
    const meta = block?.toolResult?.value?.structuredContent
    syncPanelForTool(info.name, info.args, output, meta)
}
const syncEventEffects = (event) => {
    if (event.type !== 'message') return
    const message = event.message || {}
    for (const block of blocksOf(message)) {
        if (!block) continue
        if (block.type === 'text') continue
        if (block.type === 'toolRequest') {
            const req = (block.toolCall && block.toolCall.value) || block.toolCall || {}
            const tname = String(req.name || '')
            if (tname === 'command_run' || (tname.startsWith('terminal_') && tname !== 'terminal_close')) {
                withPanel((panel) => {
                    panel.openPanel('terminal')
                    attachAgentTerminal(panel, parseJson(req.arguments), null, '')
                })
            }
            continue
        }
        if (block.type === 'toolResponse') {
            syncToolResponse(block)
            continue
        }
        if (block.type === 'actionRequired' && block.data && block.data.actionType === 'toolConfirmation') {
            const id = String(block.data.id || '')
            if (id && !notifiedActions.has(id)) {
                notifiedActions.add(id)
                notify('wait')
            }
            continue
        }
        if (block.type === 'error') {
            toast.error(`对话失败：${block.message || '未知错误'}`)
        }
    }
    if (message.metadata && message.metadata.usage) applyUsage(messageUsageDetail(message.metadata.usage), true)
    if (message.metadata && message.metadata.operations && message.metadata.operations.context && message.metadata.operations.context.compacted) loadSessionContextData()
}
const applyAgentEvent = (event) => {
    const result = reduceAgentEvent(sessionState.value, event)
    sessionState.value = result.state
    if (result.replacedTurnKey && rollbackTurnKey.value === result.replacedTurnKey.oldId) rollbackTurnKey.value = result.replacedTurnKey.newId
    if (event.type === 'usage') applyUsage(providerUsageDetail(event.usage), true)
    if (event.type === 'messageUsage') applyUsage(messageUsageDetail(event.usage), true)
    if (event.type === 'historyReplaced') { todos.value = latestTodos(sessionState.value.messages); loadSessionContextData() }
    if (event.type === 'mcpNotification' && event.server === '__context__') loadSessionContextData()
    syncEventEffects(event)
}
const finishRun = (turnKey) => {
    const sid = activeId.value
    const turn = turnOf(turnKey)
    const pending = pendingActions(sessionState.value.messages, sessionState.value.decisions).filter((item) => item.turnKey === turnKey)
    if (pending.length) {
        const status = pending.some((item) => item.toolName === 'ask_user') ? 'waiting_question' : 'waiting_approval'
        sessionState.value = markActive(sessionState.value, turnKey, status)
        notify('wait')
        if (sid) markSession(sid, 'interrupted')
        return
    }
    const errored = Boolean(turn && turn.assistant.messages.some((message) => blocksOf(message).some((block) => block && block.type === 'error')))
    if (errored) {
        sessionState.value = markActive(sessionState.value, turnKey, 'failed')
        notify('error')
        if (sid) markSession(sid, 'failed')
        return
    }
    sessionState.value = markActive(sessionState.value, turnKey, 'completed')
    notify('win')
    loadUsage()
    if (sid) markSession(sid, 'completed')
}
const activeTurn = computed(() => turns.value.find((turn) => RUNNING_STATUS.includes(String((turn.assistant && turn.assistant.status) || ''))) || null)
const isRunning = computed(() => Boolean(activeTurn.value))
const streamSessionId = ref('')
let streamController = null
const markSession = async (sid, status) => {
    if (!sid) return
    sessions.value = sessions.value.map((item) => (item.id === sid ? { ...item, status } : item))
    try {
        await updateSession(sid, { status })
    } catch { /* 静默 */ }
    if (status === 'running') return
    loadSessions().catch(() => {})
}
const runStream = async (turn, { message = '', model, agent, resume = '' } = {}) => {
    const sid = activeId.value
    if (!sid || !turn) return
    streamController = new AbortController()
    streamSessionId.value = sid
    sessionState.value = markActive(sessionState.value, turn.turnKey, 'streaming')
    try {
        await streamChat({
            message: resume ? '' : message,
            history: resume ? [] : buildHistory(turn.turnKey),
            model,
            agent,
            session_id: sid,
            context_tokens: contextTokens.value,
            context_limit: contextLimit.value,
            ...(resume ? { resume } : {}),
        }, applyAgentEvent, streamController.signal)
        finishRun(turn.turnKey)
    } catch (err) {
        if (err && (err.name === 'AbortError' || err.code === 20)) return
        const text = (err && err.message) || String(err)
        sessionState.value = markActive(sessionState.value, turn.turnKey, 'failed', { failure: { code: 'CLIENT_ERROR', message: text } })
        toast.error(`对话失败：${text}`)
        markSession(sid, 'failed')
    } finally {
        streamController = null
        streamSessionId.value = ''
    }
}
const onSend = async ({ content, model, agent, attachments = [] }) => {
    if (!projectPath.value) {
        toast.warning('请先在顶栏选择项目文件夹后再开始对话')
        return
    }
    rollbackTurnKey.value = ''
    todos.value = []
    currentModel.value = model || currentModel.value
    applyModelLimit(currentModel.value)
    const newThread = !activeId.value
    if (newThread) {
        sessionContextData.value = { summary: null, facts: [] }
        contextOthers.value = []
        syncBaselineContext([])
    }
    extractContextRefs(content).forEach(addContextFile)
    if (!activeId.value) {
        const title = content.replace(/\s+/g, ' ').trim().slice(0, 20) || '新会话'
        try {
            const data = await createSession(title)
            activeId.value = data.id
        } catch (err) {
            toast.error(`新建会话失败：${err.message}`)
        }
    }
    if (!activeId.value) {
        toast.error('新建会话失败，请重试')
        return
    }
    markSession(activeId.value, 'running')
    const storedAttachments = attachments.map((item) => ({ name: item.name, path: item.path, isImage: Boolean(item.isImage), size: Number(item.size || 0) }))
    const turnKey = `local-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
    sessionState.value = { ...sessionState.value, sessionKey: activeId.value }
    sessionState.value = appendUserMessage(sessionState.value, { id: turnKey, text: content, time: nowTime(), attachments: storedAttachments })
    listRef.value?.scrollToBottom()
    const turn = turnOf(turnKey)
    if (turn) runStream(turn, { message: content, model, agent })
}
const onStop = async ({ silent = false } = {}) => {
    const turn = activeTurn.value
    const sid = activeId.value
    if (!turn || !sid) return
    const pending = pendingActions(sessionState.value.messages, sessionState.value.decisions).filter((item) => item.turnKey === turn.turnKey)
    if (pending.length) {
        try { await approveChat(sid, 'reject') } catch { /* 409/404 视为审批已失效 */ }
    }
    try { await stopChat(sid) } catch { /* 后端已停止或不可达，仍本地中断 */ }
    if (streamController) streamController.abort()
    sessionState.value = markActive(sessionState.value, turn.turnKey, 'cancelled')
    if (!silent) {
        markSession(sid, 'interrupted')
        toast.info('已停止本轮任务')
    }
}
const resumeStream = async (turn) => {
    if (!turn || !activeId.value) return
    await runStream(turn, { model: currentModel.value, resume: activeId.value })
}
const onDecision = async (turn, part, decision) => {
    const actionId = String(part.callId || '')
    if (!activeId.value || !actionId || part.decision) return
    let approved = null
    try {
        approved = await approveChat(activeId.value, decision)
    } catch (err) {
        if (/失效|不存在|已无待决项/.test(err.message)) sessionState.value = markExpired(sessionState.value, actionId)
        toast.error(`提交审批失败：${err.message}`)
        return
    }
    const pending = pendingActions(sessionState.value.messages, sessionState.value.decisions)
    let next = sessionState.value
    for (const item of pending) next = markDecision(next, item.id, decision)
    sessionState.value = next
    if (approved && approved.subagent) return  // 子智能体内部审批：当前流仍在运行，不恢复主会话
    await resumeStream(turnOf(turn.turnKey) || turn)
}
const onAnswer = async (turn, part, answers) => {
    const actionId = String(part.callId || '')
    const list = Array.isArray(answers) ? answers : []
    if (!activeId.value || !actionId || !list.length) return
    let approved = null
    try {
        approved = await approveChat(activeId.value, 'answer', list)
    } catch (err) {
        if (/失效|不存在|已无待决项/.test(err.message)) sessionState.value = markExpired(sessionState.value, actionId)
        toast.error(`提交回答失败：${err.message}`)
        return
    }
    const pending = pendingActions(sessionState.value.messages, sessionState.value.decisions).filter((item) => item.toolName === 'ask_user')
    let next = sessionState.value
    for (const item of pending) next = markDecision(next, item.id, 'answer', list)
    if (!pending.some((item) => item.id === actionId)) next = markDecision(next, actionId, 'answer', list)
    sessionState.value = next
    if (approved && approved.subagent) return  // 子智能体内部提问：当前流仍在运行，不恢复主会话
    await resumeStream(turnOf(turn.turnKey) || turn)
}
const loadUsage = async () => {
    if (!activeId.value) { usageDetail.value = {}; contextTokens.value = 0; return }
    try {
        const data = await getSessionUsage(activeId.value)
        usageDetail.value = data || {}
        contextTokens.value = Number((data && data.total) || 0)
    } catch {
        usageDetail.value = {}
        contextTokens.value = 0
    }
}
const loadSessionContextData = async () => {
    if (!activeId.value) { sessionContextData.value = { summary: null, facts: [] }; syncBaselineContext(); return }
    try {
        const data = await getSessionContext(activeId.value)
        sessionContextData.value = { summary: (data && data.summary) || null, facts: (data && data.facts) || [] }
    } catch {
        sessionContextData.value = { summary: null, facts: [] }
    }
    syncBaselineContext()
}
const loadMcpContext = async () => {
    try {
        const data = await getMcpStatus('', false)
        const list = (data && data.list) || {}
        mcpServersActive.value = Object.entries(list).filter(([, st]) => st && st.status === 'connected').map(([name, st]) => ({ name, tools: Number(st.tools || 0) }))
    } catch {
        mcpServersActive.value = []
    }
    syncBaselineContext()
}
const onCompress = async () => {
    if (isRunning.value || isCompressing.value) { toast.warning('当前对话或压缩任务正在执行，请稍后再试'); return }
    if (!activeId.value || sessionState.value.messages.length < 2) { toast.warning('请先开始一轮对话再压缩上下文'); return }
    isCompressing.value = true
    let compactKey = ''
    try {
        const res = await compressContext({ session_id: activeId.value, history: buildHistory(), model: currentModel.value, agent: '', context_tokens: contextTokens.value, context_limit: contextLimit.value }, (event) => {
            applyAgentEvent(event)
            if (event && event.type === 'message' && event.message && event.message.id && !compactKey) {
                compactKey = String(event.message.id)
                sessionState.value = markActive(sessionState.value, compactKey, 'streaming')
                listRef.value?.scrollToBottom()
            }
        })
        if (compactKey) sessionState.value = markActive(sessionState.value, compactKey, 'completed')
        if (res && typeof res.tokens_after === 'number') await loadSessionContextData()
        await Promise.allSettled([loadUsage(), loadSessionContextData(), loadSessions()])
        toast.success('已完成上下文压缩并生成结构化摘要')
    } catch (err) {
        if (compactKey) sessionState.value = markActive(sessionState.value, compactKey, 'failed')
        toast.error(`压缩失败：${err.message}`)
    } finally {
        isCompressing.value = false
    }
}
const onRecall = (turn) => {
    rollbackTurnKey.value = turn.turnKey
}
const onAttachment = (item) => { addContextOther((item && item.name) || '', '上传附件', (item && item.path) || '') }
const onAttachmentRemoved = (item) => {
    const name = String((item && item.name) || '').trim()
    if (name) contextOthers.value = contextOthers.value.filter((entry) => entry.label !== name)
}
const onOpenFile = (payload) => {
    const rel = relOfPath((payload && payload.path) || '', projectPath.value)
    if (!rel) return
    if ((payload && payload.type) === 'folder') {
        withPanel((panel) => panel.openPanel('file'))
        return
    }
    withPanel((panel) => { panel.openPanel('file'); panel.openFile(rel) })
}
const onRollbackCancel = () => {
    rollbackTurnKey.value = ''
}
const persistTurnDeletion = async (turn, after) => {
    const sid = activeId.value
    const key = String((turn && turn.turnKey) || '')
    if (!sid || !key || key.startsWith('local-') || key.startsWith('turn-')) return true
    try {
        await deleteTurnMessages(sid, key, after)
        return true
    } catch (err) {
        if (/不存在/.test(err.message)) return true
        toast.error(`同步删除失败：${err.message}`)
        return false
    }
}
const onRollbackConfirm = async (turn) => {
    if (isRunning.value) await onStop({ silent: true })
    if (!(await persistTurnDeletion(turn, true))) return
    sessionState.value = removeTurnMessages(sessionState.value, turn.turnKey, { after: true })
    rollbackTurnKey.value = ''
    pendingDraft.value = (turn.user && turn.user.content) || ''
    syncBaselineContext()
    loadSessionContextData()
    loadSessions()
    inputRef.value && inputRef.value.focus()
}
const onDeleteTurn = async (turn) => {
    if (isRunning.value && activeTurn.value && activeTurn.value.turnKey === turn.turnKey) await onStop({ silent: true })
    if (!(await persistTurnDeletion(turn, false))) return
    sessionState.value = removeTurnMessages(sessionState.value, turn.turnKey)
    syncBaselineContext()
    loadSessionContextData()
    loadSessions()
}
const onRetryTurn = async (turn) => {
    if (!turn) return
    if (turn.isCompaction) {
        if (isCompressing.value) return
        const ok = await confirm({ title: '重试压缩', description: '确定删除当前压缩记录并重新压缩上下文吗？', confirmText: '确定重试' })
        if (!ok) return
        if (isRunning.value) await onStop({ silent: true })
        if (!(await persistTurnDeletion(turn, false))) return
        sessionState.value = removeTurnMessages(sessionState.value, turn.turnKey)
        syncBaselineContext()
        await loadSessionContextData()
        await onCompress()
        return
    }
    if (!turn.user) return
    const ok = await confirm({ title: '重试本轮', description: '确定删除当前轮次记录并重新发送该问题吗？', confirmText: '确定重试' })
    if (!ok) return
    if (isRunning.value) await onStop({ silent: true })
    if (!(await persistTurnDeletion(turn, false))) return
    const content = turn.user.content || ''
    sessionState.value = removeTurnMessages(sessionState.value, turn.turnKey)
    syncBaselineContext()
    loadSessionContextData()
    const picked = (inputRef.value && inputRef.value.currentModelPayload && inputRef.value.currentModelPayload()) || { model: currentModel.value, agent: '' }
    await onSend({ content, model: picked.model, agent: picked.agent })
}
const onContinueTurn = async () => {
    if (isRunning.value) return
    const picked = (inputRef.value && inputRef.value.currentModelPayload && inputRef.value.currentModelPayload()) || { model: currentModel.value, agent: '' }
    await onSend({ content: '参考上下文继续完成任务', model: picked.model, agent: picked.agent })
}
const resetThread = () => {
    rollbackTurnKey.value = ''
    sessionState.value = createInitialState()
    notifiedActions.clear()
    todos.value = []
    sessionContextData.value = { summary: null, facts: [] }
    contextOthers.value = []
    contextTokens.value = 0
    usageDetail.value = {}
    syncBaselineContext([])
}
const restoreFromHistory = (messages) => {
    todos.value = latestTodos(messages)
    contextOthers.value = []
    contextTokens.value = 0
    syncBaselineContext(messages)
    loadUsage()
    loadSessionContextData()
}
const loadHistory = async (sid) => {
    try {
        const data = await listMessages(sid, 0, 1000)
        const messages = ((data && data.list) || []).map((item) => item && item.message).filter(Boolean)
        sessionState.value = applyHistory({ ...createInitialState(), sessionKey: sid, sessionEpoch: Date.now() }, messages)
        notifiedActions.clear()
        rollbackTurnKey.value = ''
        restoreFromHistory(messages)
        listRef.value?.scrollToBottom()
    } catch (err) {
        toast.error(`读取历史对话失败：${err.message}`)
    }
}
const correctStaleSessions = async () => {
    const running = sessions.value.filter((item) => item.status === 'running')
    for (const session of running) {
        if (session.id === streamSessionId.value) continue
        try {
            const data = await listMessages(session.id, 0, 1000)
            const messages = ((data && data.list) || []).map((item) => item && item.message).filter(Boolean)
            const { target, label } = resolveStaleSessionTarget(messages)
            if (target !== 'running') {
                await updateSession(session.id, { status: target })
                sessions.value = sessions.value.map((item) => (item.id === session.id ? { ...item, status: target } : item))
                toast.info(`已校正「${session.title}」状态为「${label}」`)
            }
        } catch { /* 校正失败静默，保留原 running */ }
    }
}
const loadSessions = async () => {
    sessionsLoading.value = true
    try {
        const data = await listSessions()
        sessions.value = data.list || []
        if (!sessions.value.some((item) => item.id === activeId.value)) activeId.value = sessions.value[0]?.id || ''
        await correctStaleSessions()
    } catch (err) {
        toast.error(`读取会话失败：${err.message}`)
    } finally {
        sessionsLoading.value = false
    }
}
const onSelectSession = (session) => {
    if (session.id === activeId.value) return
    activeId.value = session.id
    loadHistory(session.id)
}
const onNewSession = () => {
    activeId.value = ''
    resetThread()
}
const clearDataOpen = ref(false)
const onClearData = () => { clearDataOpen.value = true }
const onDataCleared = async () => {
    activeId.value = ''
    resetThread()
    await loadSessions()
}
const renameOpen = ref(false)
const renameSaving = ref(false)
const renameText = ref('')
const renameTarget = ref(null)
const onRenameSession = (session) => {
    renameTarget.value = session
    renameText.value = String((session && session.title) || '')
    renameOpen.value = true
}
const onRenameConfirm = async () => {
    const session = renameTarget.value
    const value = String(renameText.value || '').trim()
    if (!value) { toast.warning('标题不能为空'); return }
    if (!session) return
    renameSaving.value = true
    try {
        await updateSession(session.id, { title: value })
        renameOpen.value = false
        await loadSessions()
    } catch (err) {
        toast.error(`重命名失败：${err.message || err}`)
    } finally {
        renameSaving.value = false
    }
}
const onDeleteSession = async (session) => {
    const ok = await confirm({ title: '删除会话', description: `删除会话「${session.title}」？`, confirmText: '删除', destructive: true })
    if (!ok) return
    try {
        const wasActive = activeId.value === session.id
        await deleteSession(session.id)
        await loadSessions()
        if (wasActive) {
            activeId.value = sessions.value[0]?.id || ''
            if (activeId.value) loadHistory(activeId.value)
            else resetThread()
        }
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const pickProject = async () => {
    try {
        const response = await fetch(`${API_BASE}/api/app/pick_folder`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' })
        const body = await response.json().catch(() => null)
        const path = String((body && body.data && body.data.path) || (body && body.path) || '')
        if (!response.ok || !path) {
            toast.warning('请在桌面端中选择文件夹')
            return
        }
        projectPath.value = path
        setProject(path).catch(() => {})
    } catch {
        toast.warning('请在桌面端中选择文件夹')
    }
}
const loadChatPrefs = async () => {
    try {
        const data = await getChatSse()
        const notifyPrefs = (data && data.notify) || {}
        chatPrefs.value = { ask: data.ask !== false, todo: data.todo !== false, notify: { banner: notifyPrefs.banner !== false, sound: notifyPrefs.sound !== false, volume: Number(notifyPrefs.volume ?? 100) } }
        if (!chatPrefs.value.todo) todos.value = []
    } catch { /* 读取失败保持默认 */ }
}
const loadSkills = async () => {
    try {
        const data = await listSkills()
        skillCatalog.value = (data.list || []).filter((item) => item.enabled).map((item) => ({ name: item.name, description: item.description || '' }))
    } catch { skillCatalog.value = [] }
    syncBaselineContext()
}
const loadRules = async () => {
    try {
        const data = await listRelus()
        ruleCatalog.value = Array.isArray((data && data.list) || []) ? data.list : []
    } catch { /* 读取失败忽略 */ }
    syncBaselineContext()
}
const loadModels = async () => {
    try {
        const data = await listModels()
        modelList.value = (data.list || []).map((item) => ({ provider: item.provider || '', model: item.model, context: item.context || '', enabled: item.enabled !== false }))
        const first = modelList.value.find((item) => item.enabled)
        if (!currentModel.value && first) { currentModel.value = first.model; currentProvider.value = first.provider }
        if (currentModel.value) applyModelLimit(currentModel.value, currentProvider.value)
    } catch { modelList.value = [] }
}
const signalAppReady = () => {
    nextTick(() => {
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                window.dispatchEvent(new Event('lwa:app-ready'))
            })
        })
    })
}
const onModelParamSaved = () => loadModels()
onMounted(async () => {
    sideWidth.value = clampSide(getNumber(SIDE_KEY, SIDE_DEFAULT))
    const storedRight = getNumber(RIGHT_KEY, 0)
    if (storedRight > 0) {
        rightWidth.value = clampRight(storedRight)
        rightFollowRatio.value = false
    } else {
        rightWidth.value = defaultRightWidth()
        rightFollowRatio.value = true
    }
    window.addEventListener('resize', onWindowResize)
    window.addEventListener('lwa:layout-remember', onLayoutRemember)
    window.addEventListener('lwa:new-chat', onNewSession)
    window.addEventListener('lwa:clear-data', onClearData)
    window.addEventListener('lwa:model-param-saved', onModelParamSaved)
    await Promise.allSettled([
        restoreLayout(),
        loadSessions().then(() => (activeId.value ? loadHistory(activeId.value) : null)),
        getProject().then((data) => { if (data && data.path) projectPath.value = data.path }).catch(() => {}),
        loadBuiltTools(),
        loadChatPrefs(),
        loadSkills(),
        loadModels(),
        loadRules(),
        loadMcpContext(),
    ])
    signalAppReady()
})
onBeforeUnmount(() => {
    window.removeEventListener('lwa:new-chat', onNewSession)
    window.removeEventListener('lwa:clear-data', onClearData)
    window.removeEventListener('resize', onWindowResize)
    window.removeEventListener('lwa:layout-remember', onLayoutRemember)
    window.removeEventListener('lwa:model-param-saved', onModelParamSaved)
    clearTimeout(rightResizeTimer)
    clearTimeout(layoutSaveTimer)
})
watch(setingOpen, (value) => { if (!value) { loadChatPrefs(); loadBuiltTools(); loadSkills(); loadRules(); loadMcpContext(); loadModels(); window.dispatchEvent(new Event('lwa:models-changed')) } })
watch(rightOpen, () => { fitWidths(); scheduleLayoutSave() })
watch([sideWidth, rightWidth, rightTab], scheduleLayoutSave)
</script>

<style scoped>
.chat-page { height: 100vh; padding: 12px 12px; }
.chat-splitter { display: flex; align-items: stretch; height: 100%; min-width: 0; }
.chat-bar { position: relative; flex: none; width: 12px; background: transparent; cursor: col-resize; }
.chat-bar::before { content: ''; position: absolute; top: 0; bottom: 0; left: 50%; width: 1px; background: var(--border); transform: translateX(-50%); transition: background 0.15s ease, width 0.15s ease; }
.chat-bar:hover::before, .chat-bar:focus-visible::before { width: 2px; background: var(--primary); }
.chat-bar:focus-visible { outline: none; }
.chat-side-panel { flex: none; height: 100%; overflow: hidden; }
.chat-main-panel { flex: 1 1 0; height: 100%; min-width: 0; overflow: hidden; }
.chat-right-panel { flex: none; height: 100%; overflow: hidden; }
.chat-side { display: flex; flex-direction: column; gap: 10px; height: 100%; min-height: 0; padding-right: 12px; }
.side-top { display: flex; flex-direction: column; gap: 8px; }
.new-chat { width: 100%; height: 40px; padding-left: 14px; border-radius: 8px; justify-content: flex-start; }
.side-foot { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 6px; padding: 6px 2px 0; border-top: 1px solid var(--border); }
.foot-gear { width: 24px; min-width: 24px; height: 24px; padding: 0; }
.chat-main { display: flex; flex-direction: column; gap: 10px; height: 100%; min-width: 0; padding: 4px 8px 8px; }
.chat-topbar { display: flex; flex: none; align-items: center; justify-content: space-between; gap: 12px; height: 40px; padding: 0 4px; }
.project-switch { display: inline-flex; align-items: center; gap: 6px; padding: 4px 6px; border: none; border-radius: 6px; background: transparent; color: var(--foreground); font-size: 13px; font-weight: 600; cursor: pointer; }
.project-switch:hover { background: var(--accent); }
.project-switch.missing { color: #e6a23c; }
.project-switch.missing:hover { background: color-mix(in srgb, #e6a23c 12%, transparent); }
.panel-toggle { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; padding: 0; border: none; border-radius: 6px; background: transparent; color: var(--muted-foreground); cursor: pointer; }
.panel-toggle:hover { background: var(--accent); color: var(--foreground); }
</style>