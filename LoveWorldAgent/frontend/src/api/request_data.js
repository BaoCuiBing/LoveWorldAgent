// 规则接口封装：开发态走 Vite 代理，生产态（Tauri 加载 8866）直连本机 HTTP 服务
const BASE = import.meta.env.DEV ? '' : 'http://127.0.0.1:8866'
const request = async (path, options = {}) => {
    const response = await fetch(`${BASE}${path}`, { headers: { 'Content-Type': 'application/json' }, ...options })
    const body = await response.json().catch(() => null)
    if (response.status === 404 && !body && (path.startsWith('/api/relus') || path.startsWith('/api/skills') || path.startsWith('/api/chat') || path.startsWith('/api/models') || path.startsWith('/api/mcp') || path.startsWith('/api/win'))) throw new Error('后端未加载对应接口，请重启应用后重试')
    if (!response.ok || !body || body.code !== 200) throw new Error((body && body.msg) || `HTTP ${response.status}`)
    return body.data
}
export const checkHealth = async () => {
    try {
        const response = await fetch(`${BASE}/api/health`)
        const body = await response.json().catch(() => null)
        return Boolean(response.ok && body && body.status === 'healthy')
    } catch {
        return false
    }
}
export const listRelus = () => request('/api/relus')
export const readRelu = (name) => request(`/api/relus/${encodeURIComponent(name)}`)
export const createRelu = (name, content, trigger, description) => request('/api/relus', { method: 'POST', body: JSON.stringify({ name, content, trigger, description }) })
export const updateRelu = (name, content, trigger, description) => request(`/api/relus/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify({ content, trigger, description }) })
export const deleteRelu = (name) => request(`/api/relus/${encodeURIComponent(name)}`, { method: 'DELETE' })
export const listSkills = (q = '') => request(`/api/skills${q ? `?q=${encodeURIComponent(q)}` : ''}`)
export const readSkill = (name) => request(`/api/skills/${encodeURIComponent(name)}`)
export const createSkill = (payload) => request('/api/skills', { method: 'POST', body: JSON.stringify(payload) })
export const updateSkill = (name, payload) => request(`/api/skills/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify(payload) })
export const deleteSkill = (name) => request(`/api/skills/${encodeURIComponent(name)}`, { method: 'DELETE' })
export const setSkillEnabled = (name, enabled) => request(`/api/skills/${encodeURIComponent(name)}/enabled`, { method: 'PATCH', body: JSON.stringify({ enabled }) })
export const getChatSse = () => request('/api/chat/sse')
export const saveChatSse = (chat) => request('/api/chat/sse', { method: 'PUT', body: JSON.stringify(chat) })
export const approveChat = (runId, decision, answers) => request('/api/chat/approve', { method: 'POST', body: JSON.stringify(answers && answers.length ? { run_id: runId, decision, answers } : { run_id: runId, decision }) })
export const stopChat = (runId) => request('/api/chat/stop', { method: 'POST', body: JSON.stringify({ run_id: runId }) })
export const chatSoundUrl = (name) => `${BASE}/api/chat/sse/sound/${encodeURIComponent(name)}`
const readSse = async (response, onEvent) => {
    if (!response.ok) {
        const body = await response.json().catch(() => null)
        throw new Error((body && body.msg) || `HTTP ${response.status}`)
    }
    if (!response.body) throw new Error('响应体为空')
    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    for (;;) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })
        let index
        while ((index = buffer.indexOf('\n\n')) !== -1) {
            const chunk = buffer.slice(0, index)
            buffer = buffer.slice(index + 2)
            const line = chunk.split('\n').find((l) => l.startsWith('data: '))
            if (!line) continue
            try { onEvent && onEvent(JSON.parse(line.slice(6))) } catch { /* 忽略坏帧 */ }
        }
    }
}
export const compressContext = async (payload, onEvent = null, signal = null) => {
    const response = await fetch(`${BASE}/api/chat/compress`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload), signal })
    const ctype = String(response.headers.get('content-type') || '').toLowerCase()
    if (!response.ok || ctype.includes('application/json')) {
        const body = await response.json().catch(() => null)
        if (!response.ok || !body || body.code !== 200) throw new Error((body && body.msg) || `HTTP ${response.status}`)
        return body.data
    }
    await readSse(response, onEvent)
    return null
}
export const getSessionUsage = (id) => request(`/api/sessions/${encodeURIComponent(id)}/usage`)
export const getSessionContext = (id) => request(`/api/sessions/${encodeURIComponent(id)}/context`)
// SSE 对话流：data 是 goose AgentEvent，字段为 event.type（message/usage/messageUsage/mcpNotification/historyReplaced）
export const streamChat = async (payload, onEvent, signal) => {
    const response = await fetch(`${BASE}/api/chat/stream`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload), signal })
    await readSse(response, onEvent)
}
export const listSessions = (q = '') => request(`/api/sessions${q ? `?q=${encodeURIComponent(q)}` : ''}`)
export const createSession = (title) => request('/api/sessions', { method: 'POST', body: JSON.stringify({ title }) })
export const updateSession = (id, payload) => request(`/api/sessions/${encodeURIComponent(id)}`, { method: 'PATCH', body: JSON.stringify(payload) })
export const deleteSession = (id) => request(`/api/sessions/${encodeURIComponent(id)}`, { method: 'DELETE' })
export const getProject = () => request('/api/project')
export const setProject = (path) => request('/api/project', { method: 'PUT', body: JSON.stringify({ path }) })
export const projectFiles = (path = '', q = '') => request(`/api/project/files?path=${encodeURIComponent(path)}${q ? `&q=${encodeURIComponent(q)}` : ''}`)
export const projectFile = (path) => request(`/api/project/file?path=${encodeURIComponent(path)}`)
export const projectFileRawUrl = (path) => `${BASE}/api/project/file/raw?path=${encodeURIComponent(path)}`
export const projectFileRaw = async (path) => {
    const response = await fetch(projectFileRawUrl(path))
    if (!response.ok) {
        const body = await response.json().catch(() => null)
        throw new Error((body && body.msg) || `HTTP ${response.status}`)
    }
    return await response.arrayBuffer()
}
export const saveProjectFile = (path, content) => request('/api/project/file', { method: 'PUT', body: JSON.stringify({ path, content }) })
export const statProjectEntry = (path) => request(`/api/project/entry?path=${encodeURIComponent(path)}`)
export const createProjectEntry = (path, type = 'file') => request('/api/project/entry', { method: 'POST', body: JSON.stringify({ path, type }) })
export const deleteProjectEntry = (path) => request(`/api/project/entry?path=${encodeURIComponent(path)}`, { method: 'DELETE' })
export const copyProjectEntry = (src, dest) => request('/api/project/copy', { method: 'POST', body: JSON.stringify({ src, dest }) })
export const listMessages = (id, afterSeq = 0, limit = 50) => request(`/api/sessions/${encodeURIComponent(id)}/messages?after_seq=${afterSeq}&limit=${limit}`)
export const appendMessage = (id, payload) => request(`/api/sessions/${encodeURIComponent(id)}/messages`, { method: 'POST', body: JSON.stringify(payload) })
export const deleteTurnMessages = (id, turnKey, after = false) => request(`/api/sessions/${encodeURIComponent(id)}/messages/${encodeURIComponent(turnKey)}?after=${after ? '1' : '0'}`, { method: 'DELETE' })
export const openTerminal = (payload) => request('/api/terminal/open', { method: 'POST', body: JSON.stringify(payload) })
export const setTerminalPersistent = (id, persistent = true, command = '') => request('/api/terminal/persistent', { method: 'POST', body: JSON.stringify({ id, persistent, command }) })
export const listTerminalSessions = (kind = '') => request(`/api/terminal/sessions${kind ? `?kind=${encodeURIComponent(kind)}` : ''}`)
export const terminalInput = (payload) => request('/api/terminal/input', { method: 'POST', body: JSON.stringify(payload) })
export const permissionCheck = (command) => request('/api/permission/check', { method: 'POST', body: JSON.stringify({ command }) })
export const permissionAllow = (command) => request('/api/permission/allow', { method: 'POST', body: JSON.stringify({ command }) })
export const terminalResize = (payload) => request('/api/terminal/resize', { method: 'POST', body: JSON.stringify(payload) })
export const closeTerminal = (id) => request('/api/terminal/close', { method: 'POST', body: JSON.stringify({ id }) })
export const terminalStreamUrl = (id) => `${BASE}/api/terminal/stream?id=${encodeURIComponent(id)}`
// 终端长连接：一个标签一条 WebSocket，开发态同源走 Vite 代理（proxy 已开 ws），打包态 file:// 直连 8866
export const terminalWsUrl = (query = '') => {
    const scheme = location.protocol === 'https:' ? 'wss' : 'ws'
    const base = import.meta.env.DEV ? `${scheme}://${location.host}` : 'ws://127.0.0.1:8866'
    return `${base}/api/terminal/ws${query ? `?${query}` : ''}`
}
export const startBrowser = (rect, url = '') => request('/api/browser/start', { method: 'POST', body: JSON.stringify({ rect, url }) })
export const updateBrowserRect = (rect) => request('/api/browser/rect', { method: 'POST', body: JSON.stringify({ rect }) })
export const navigateBrowser = (url) => request('/api/browser/navigate', { method: 'POST', body: JSON.stringify({ url }) })
export const showBrowser = (value) => request('/api/browser/show', { method: 'POST', body: JSON.stringify({ value }) })
export const browserState = () => request('/api/browser/state')
export const stopBrowser = () => request('/api/browser/stop', { method: 'POST' })
export const browserTool = (name, args = {}, timeout = 15) => request('/api/browser/tool', { method: 'POST', body: JSON.stringify({ name, args, timeout }) })
export const getBrowserSettings = () => request('/api/browser/settings')
export const saveBrowserSettings = (payload) => request('/api/browser/settings', { method: 'PUT', body: JSON.stringify(payload) })
export const clearScreenshots = () => request('/api/browser/screenshot/clear', { method: 'POST' })
export const getImportSources = () => request('/api/browser/import/sources')
export const importBrowserCookies = (source, includeExpired = false) => request('/api/browser/import/browser', { method: 'POST', body: JSON.stringify({ source, include_expired: includeExpired }) })
export const browserSteps = (after = 0) => request(`/api/browser/steps?after=${encodeURIComponent(after)}`)
export const browserScreenshot = (path) => request(`/api/browser/screenshot?path=${encodeURIComponent(path)}`)
export const getTheme = () => request('/api/system/theme')
export const saveTheme = (theme) => request('/api/system/theme', { method: 'PUT', body: JSON.stringify({ theme }) })
export const getLayout = () => request('/api/system/layout')
export const saveLayout = (layout) => request('/api/system/layout', { method: 'PUT', body: JSON.stringify(layout) })
export const getAgentSettings = () => request('/api/agent/settings')
export const saveAgentSettings = (payload) => request('/api/agent/settings', { method: 'PUT', body: JSON.stringify(payload) })
export const updateWindow = (theme, repaint = '') => request('/api/win/update', { method: 'POST', body: JSON.stringify(repaint ? { theme, repaint } : { theme }) })
export const listModels = () => request('/api/models')
export const createModel = (payload) => request('/api/models', { method: 'POST', body: JSON.stringify(payload) })
export const updateModel = (index, payload) => request(`/api/models/${index}`, { method: 'PUT', body: JSON.stringify(payload) })
export const deleteModel = (index) => request(`/api/models/${index}`, { method: 'DELETE' })
export const setModelEnabled = (index, enabled) => request(`/api/models/${index}/enabled`, { method: 'PATCH', body: JSON.stringify({ enabled }) })
export const getPermission = () => request('/api/permission')
export const savePermission = (payload) => request('/api/permission', { method: 'PUT', body: JSON.stringify(payload) })
export const getComputerUse = () => request('/api/computer-use')
export const saveComputerUse = (payload) => request('/api/computer-use', { method: 'PUT', body: JSON.stringify(typeof payload === 'boolean' ? { enabled: payload } : payload) })
export const computerUseState = () => request('/api/computer-use/state')
export const computerUseSteps = (after = 0) => request(`/api/computer-use/steps?after=${encodeURIComponent(after)}`)
export const computerUseScreenshot = (path) => request(`/api/computer-use/screenshot?path=${encodeURIComponent(path)}`)
export const clearComputerUseScreenshots = () => request('/api/computer-use/screenshot/clear', { method: 'POST' })
export const stopComputerUse = () => request('/api/computer-use/stop', { method: 'POST' })
export const getWordOption = () => request('/api/word-option')
export const saveWordOption = (payload) => request('/api/word-option', { method: 'PUT', body: JSON.stringify(payload) })
export const getPptOption = () => request('/api/ppt-option')
export const savePptOption = (payload) => request('/api/ppt-option', { method: 'PUT', body: JSON.stringify(payload) })
export const getExcelOption = () => request('/api/excel-option')
export const saveExcelOption = (payload) => request('/api/excel-option', { method: 'PUT', body: JSON.stringify(payload) })
export const officeOptionState = () => request('/api/office-option/state')
export const officeOptionSteps = (after = 0) => request(`/api/office-option/steps?after=${encodeURIComponent(after)}`)
export const officeOptionScreenshot = (path) => request(`/api/office-option/screenshot?path=${encodeURIComponent(path)}`)
export const clearOfficeOptionScreenshots = () => request('/api/office-option/screenshot/clear', { method: 'POST' })
export const stopOfficeOption = () => request('/api/office-option/stop', { method: 'POST' })
export const getMobileOption = () => request('/api/mobile-option')
export const saveMobileOption = (payload) => request('/api/mobile-option', { method: 'PUT', body: JSON.stringify(payload) })
export const mobileOptionState = () => request('/api/mobile-option/state')
export const mobileOptionSteps = (after = 0) => request(`/api/mobile-option/steps?after=${encodeURIComponent(after)}`)
export const stopMobileOption = () => request('/api/mobile-option/stop', { method: 'POST' })
export const clearDataPreview = () => request('/api/data/clear')
export const clearData = () => request('/api/data/clear', { method: 'POST' })
export const quitApp = () => request('/api/app/quit', { method: 'POST' })
export const exitApp = () => request('/api/app/exit', { method: 'POST' })
export const openDoc = (name) => request('/api/open', { method: 'POST', body: JSON.stringify({ name }) })
export const listAgents = () => request('/api/agents')
export const getAgent = (id) => request(`/api/agents/${encodeURIComponent(id)}`)
export const createAgent = (payload) => request('/api/agents', { method: 'POST', body: JSON.stringify(payload) })
export const updateAgent = (id, payload) => request(`/api/agents/${encodeURIComponent(id)}`, { method: 'PUT', body: JSON.stringify(payload) })
export const deleteAgent = (id) => request(`/api/agents/${encodeURIComponent(id)}`, { method: 'DELETE' })
export const setAgentEnabled = (id, enabled) => request(`/api/agents/${encodeURIComponent(id)}/enabled`, { method: 'PATCH', body: JSON.stringify({ enabled }) })
export const uploadFile = async (file) => {
    const form = new FormData()
    form.append('file', file)
    const response = await fetch(`${BASE}/api/upload`, { method: 'POST', body: form })
    const body = await response.json().catch(() => null)
    if (!response.ok || !body || body.code !== 200) throw new Error((body && body.msg) || `HTTP ${response.status}`)
    return body.data
}
export const deleteUpload = (path) => request(`/api/upload?path=${encodeURIComponent(path || '')}`, { method: 'DELETE' })
export const uploadPreviewUrl = (name) => `${BASE}/api/upload/preview?name=${encodeURIComponent(name || '')}`
export const listMcp = () => request('/api/mcp')
export const getMcpStatus = (name = '', refresh = false) => {
    const params = new URLSearchParams()
    if (name) params.set('name', name)
    if (refresh) params.set('refresh', '1')
    const query = params.toString()
    return request(`/api/mcp/status${query ? `?${query}` : ''}`)
}
export const createMcp = (payload) => request('/api/mcp', { method: 'POST', body: JSON.stringify(payload) })
export const updateMcp = (name, payload) => request(`/api/mcp/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify(payload) })
export const deleteMcp = (name) => request(`/api/mcp/${encodeURIComponent(name)}`, { method: 'DELETE' })
export const setMcpEnabled = (name, enabled) => request(`/api/mcp/${encodeURIComponent(name)}/enabled`, { method: 'PATCH', body: JSON.stringify({ enabled }) })
export const getMcpJson = () => request('/api/mcp/json')
export const saveMcpJson = (text) => request('/api/mcp/json', { method: 'PUT', body: JSON.stringify({ text }) })
