/** goose 内容块 → 现有卡片所需字段的映射层：
 *  text → markdown、thinking → thinking、toolRequest/toolResponse → tool、actionRequired → confirmation/question、
 *  todo_write → todo、error → error，其余（image/document/systemNotification/redactedThinking/elicitation）用 UnsupportedCard 兜底。
 *  只面向新契约（status/meta/arguments/output/actionRequired），不做旧字段回退；子智能体消息在本层按 callId 归为 subagent 卡。 */
import { ASK_TOOL, actionOf, blocksOf, isHiddenMessage, permissionToDecision } from '../../utils/chatContract.js'
const PLAIN = (value) => Boolean(value) && typeof value === 'object' && !Array.isArray(value)
const textOfContent = (items) => (Array.isArray(items) ? items : []).map((block) => {
    if (!block) return ''
    if (typeof block === 'string') return block
    if (block.type === 'text') return String(block.text || '')
    if (block.type === 'resource') {
        const resource = block.resource || {}
        return String(resource.text || resource.blob || '')
    }
    return JSON.stringify(block, null, 2)
}).filter(Boolean).join('\n')
/** CallToolResult 的文本正文：优先 text 内容块，其次 structuredContent，最后整体 JSON */
export const toolResponseOutput = (block) => {
    const result = block && block.toolResult ? block.toolResult : {}
    if (result.status === 'error') return String(result.error || '')
    const value = result.value
    if (value === undefined || value === null) return ''
    if (Array.isArray(value)) return textOfContent(value)
    if (PLAIN(value)) {
        const text = textOfContent(value.content)
        if (text) return text
        if (value.structuredContent !== undefined && value.structuredContent !== null) return JSON.stringify(value.structuredContent, null, 2)
        return JSON.stringify(value, null, 2)
    }
    return String(value)
}
/** toolResponse 的旁路元数据（structuredContent）：file_write 的 diff 明细 */
const toolResponseMeta = (block) => {
    const value = block && block.toolResult ? block.toolResult.value : null
    return PLAIN(value) && PLAIN(value.structuredContent) ? value.structuredContent : null
}
/** 收集全量 toolResponse：id → { status, output, isError, meta } */
const collectToolResponses = (messages) => {
    const map = new Map()
    for (const message of messages || []) {
        for (const block of blocksOf(message)) {
            if (!block || block.type !== 'toolResponse' || !block.id) continue
            const result = block.toolResult || {}
            const meta = toolResponseMeta(block)
            const hasBadExit = meta && typeof meta.exitCode === 'number' && meta.exitCode !== 0
            const isError = result.status === 'error'
                || (result.status === 'success' && Boolean(result.value && result.value.isError))
                || Boolean(meta && meta.ok === false)
                || Boolean(hasBadExit)
            map.set(String(block.id), { status: isError ? 'failed' : 'completed', output: toolResponseOutput(block), isError, meta })
        }
    }
    return map
}
const collectPermissions = (messages) => {
    const map = new Map()
    for (const message of messages || []) {
        for (const block of blocksOf(message)) {
            if (!block || block.type !== 'actionRequired' || !block.data || block.data.actionType !== 'toolConfirmationResponse' || !block.data.id) continue
            map.set(String(block.data.id), block.data.permission)
        }
    }
    return map
}
const collectActions = (messages) => {
    const map = new Map()
    for (const message of messages || []) {
        for (const block of blocksOf(message)) {
            const action = actionOf(block)
            if (action && action.id && !map.has(String(action.id))) map.set(String(action.id), action)
        }
    }
    return map
}
/** 在消息列表中定位某个工具请求（工具响应消息只带 id，需要回查名称与参数） */
export const findToolRequest = (messages, id) => {
    const target = String(id || '')
    if (!target) return null
    for (let index = (messages || []).length - 1; index >= 0; index -= 1) {
        for (const block of blocksOf(messages[index])) {
            if (!block || block.type !== 'toolRequest' || String(block.id || '') !== target) continue
            const call = block.toolCall || {}
            const value = call.status === 'success' && PLAIN(call.value) ? call.value : {}
            return { id: target, name: String(value.name || ''), args: PLAIN(value.arguments) ? value.arguments : {}, error: call.status === 'error' ? String(call.error || '') : '' }
        }
    }
    return null
}
const parseJson = (value) => {
    if (PLAIN(value) || Array.isArray(value)) return value
    if (typeof value !== 'string' || !value.trim()) return null
    try { return JSON.parse(value) } catch { return null }
}
const questionList = (args) => {
    const parsed = parseJson(args && args.questions)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(PLAIN).map((item) => ({
        question: String(item.question || ''),
        options: Array.isArray(item.options) ? item.options.map((option) => String(option)) : [],
        mode: item.mode === 'multiple' ? 'multiple' : 'single',
        customNote: Boolean(item.customNote),
    }))
}
const answerList = (text) => {
    const parsed = parseJson(text)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(PLAIN).map((item) => ({ question: String(item.question || ''), answer: String(item.answer || item.text || ''), note: String(item.note || ''), skipped: Boolean(item.skipped) }))
}
const todoRows = (args, output) => {
    const parsed = parseJson(output) || parseJson(args && args.items) || parseJson(args && args.todos) || []
    const rows = Array.isArray(parsed) ? parsed : []
    return rows.filter(PLAIN).map((item) => ({ content: String(item.content || item.title || ''), status: String(item.status || 'pending') })).filter((item) => item.content)
}
/** 会话里最后一次 todo_write 的清单，用于切换会话时恢复右栏待办 */
export const latestTodos = (messages) => {
    const requests = new Map()
    let rows = []
    for (const message of messages || []) {
        for (const block of blocksOf(message)) {
            if (!block) continue
            if (block.type === 'toolRequest' && block.id) {
                const call = block.toolCall || {}
                const value = call.status === 'success' && PLAIN(call.value) ? call.value : {}
                if (value.name === 'todo_write') requests.set(String(block.id), PLAIN(value.arguments) ? value.arguments : {})
                continue
            }
            if (block.type !== 'toolResponse' || !block.id || !requests.has(String(block.id))) continue
            const parsed = todoRows(requests.get(String(block.id)), toolResponseOutput(block))
            if (parsed.length) rows = parsed
        }
    }
    return rows
}
const commandOf = (args) => {
    const value = PLAIN(args) ? args : {}
    for (const key of ['command', 'cmd', 'code', 'script', 'path']) {
        const text = String(value[key] || '').trim()
        if (text) return text
    }
    const raw = JSON.stringify(value)
    return raw === '{}' ? '' : raw
}
const mergeAdjacent = (parts) => {
    const out = []
    for (const part of parts) {
        const last = out[out.length - 1]
        if (last && part.type === 'markdown' && last.type === 'markdown') {
            last.content += part.content
            continue
        }
        if (last && part.type === 'thinking' && last.type === 'thinking') {
            last.content += part.content
            continue
        }
        out.push({ ...part })
    }
    return out
}
const callValueOf = (block) => {
    const call = block && block.toolCall ? block.toolCall : {}
    return call.status === 'success' && PLAIN(call.value) ? call.value : {}
}
/** 子智能体元数据：metadata.operations.subagent 优先，兼容 metadata.subagent */
const subagentOfMessage = (message) => {
    const meta = message && message.metadata
    if (!PLAIN(meta)) return null
    const operations = PLAIN(meta.operations) ? meta.operations : parseJson(meta.operations)
    const legacy = PLAIN(meta.subagent) ? meta.subagent : parseJson(meta.subagent)
    const sub = (PLAIN(operations) && PLAIN(operations.subagent) ? operations.subagent : null) || (PLAIN(legacy) ? legacy : null)
    return sub && sub.callId ? sub : null
}
const tailVisibleId = (messages) => {
    for (let index = (messages || []).length - 1; index >= 0; index -= 1) {
        const message = messages[index]
        if (message && !isHiddenMessage(message)) return String(message.id || '')
    }
    return ''
}
const hasUnresolvedTool = (messages, responses) => (messages || []).some((message) => blocksOf(message).some((block) => {
    if (!block || block.type !== 'toolRequest' || !block.id) return false
    return (block.toolCall || {}).status !== 'error' && !responses.has(String(block.id))
}))
const hasFailedTool = (messages, responses) => (messages || []).some((message) => blocksOf(message).some((block) => {
    if (!block) return false
    if (block.type === 'error') return true
    if (block.type !== 'toolRequest' || !block.id) return false
    if ((block.toolCall || {}).status === 'error') return true
    const response = responses.get(String(block.id))
    return Boolean(response && (response.status === 'failed' || response.isError))
}))
/** 子智能体卡片：携带该组原始消息，卡片展开时再逐条取 parts；actionRequired 不进入卡片 */
const cardOfGroup = (group, context) => {
    const { streaming, responses, tailId } = context
    const messages = group.messages.filter((message) => blocksOf(message).some((block) => block && block.type !== 'actionRequired'))
    const lastId = String((group.messages[group.messages.length - 1] || {}).id || '')
    const pending = Boolean(streaming) || hasUnresolvedTool(group.messages, responses)
    let toolCount = 0
    group.messages.forEach((message) => blocksOf(message).forEach((block) => {
        if (block && block.type === 'toolRequest' && String(callValueOf(block).name || '') !== ASK_TOOL) toolCount += 1
    }))
    return {
        type: 'subagent',
        partKey: `agent:${group.callId}`,
        agentId: group.agentId,
        agentName: group.agentName,
        messages,
        running: Boolean(lastId) && lastId === tailId && pending,
        failed: hasFailedTool(messages, responses),
        toolCount,
    }
}
/** 未被卡片占用的 agent_call 工具卡：统一退化为空 subagent 卡，避免落到未支持卡 */
const cardOfCall = (part) => {
    if (!part || part.type !== 'tool' || part.toolName !== 'agent_call') return null
    const meta = PLAIN(part.meta) ? part.meta : {}
    const args = parseJson(part.arguments) || {}
    const agentId = String(meta.agentId || args.agent_id || '')
    return {
        type: 'subagent',
        partKey: part.partKey,
        agentId,
        agentName: String(meta.agentName || args.agent_id || ''),
        messages: [],
        running: part.status === 'running' || part.status === 'pending',
        failed: part.status === 'failed' || part.status === 'terminated' || meta.ok === false,
        toolCount: 0,
    }
}
/** 卡片锚点：优先 meta.callId 精确匹配的 agent_call 工具卡，否则取同 agentId 且位置最近的 agent_call */
const insertAgentCards = (parts, groups, agentCalls, context) => {
    const cards = new Map()
    groups.forEach((group, callId) => { cards.set(callId, cardOfGroup(group, context)) })
    if (!cards.size && !agentCalls.length) return parts
    const indexOf = new Map()
    parts.forEach((part, index) => { if (part && part.partKey) indexOf.set(part.partKey, index) })
    const used = new Set()
    const replaceAt = new Map()
    const insertAt = new Map()
    const pending = []
    cards.forEach((card, callId) => {
        const exact = agentCalls.findIndex((call) => call.callId && call.callId === callId)
        const at = exact >= 0 ? indexOf.get(agentCalls[exact].partKey) : undefined
        if (exact >= 0 && at !== undefined && !used.has(exact)) {
            used.add(exact)
            replaceAt.set(at, card)
            return
        }
        pending.push({ group: groups.get(callId), card })
    })
    pending.forEach(({ group, card }) => {
        const anchor = group.anchorKey && indexOf.has(group.anchorKey) ? indexOf.get(group.anchorKey) : -1
        const start = anchor + 1
        let best = -1
        let distance = Infinity
        agentCalls.forEach((call, index) => {
            if (used.has(index)) return
            if (call.agentId !== group.agentId) return
            const at = indexOf.get(call.partKey)
            if (at === undefined) return
            if (Math.abs(at - anchor) < distance) { best = index; distance = Math.abs(at - anchor) }
        })
        if (best >= 0) {
            used.add(best)
            replaceAt.set(indexOf.get(agentCalls[best].partKey), card)
            return
        }
        if (!insertAt.has(start)) insertAt.set(start, [])
        insertAt.get(start).push(card)
    })
    const out = []
    for (let index = 0; index <= parts.length; index += 1) {
        if (insertAt.has(index)) out.push(...insertAt.get(index))
        if (index === parts.length) break
        const part = parts[index]
        if (replaceAt.has(index)) { out.push(replaceAt.get(index)); continue }
        out.push(cardOfCall(part) || part)
    }
    return out
}
export const partsOfMessages = (messages, options = {}) => {
    const { sessionId = '', decisions = {}, expired = {}, streaming = false, ignoreSubagent = false } = options
    const groups = new Map()
    const groupOf = new Map()
    if (!ignoreSubagent) {
        for (const message of messages || []) {
            const sub = subagentOfMessage(message)
            const id = String((message && message.id) || '')
            if (!sub || !id) continue
            const callId = String(sub.callId)
            if (!groups.has(callId)) groups.set(callId, { callId, agentId: '', agentName: '', messages: [], anchorKey: '' })
            const group = groups.get(callId)
            if (!group.agentId && sub.agentId) group.agentId = String(sub.agentId)
            if (!group.agentName && sub.agentName) group.agentName = String(sub.agentName)
            group.messages.push(message)
            groupOf.set(id, group)
        }
    }
    const responses = collectToolResponses(messages)
    const permissions = collectPermissions(messages)
    const actions = collectActions(messages)
    const requestIds = new Set()
    const parts = []
    const tailParts = []
    const agentCalls = []
    const actionPart = (action, partKey) => {
        const id = String(action.id || '')
        const local = decisions[id] || {}
        const response = responses.get(id)
        if (String(action.toolName || '') === ASK_TOOL) {
            const answers = Array.isArray(local.answers) && local.answers.length ? local.answers : answerList(response ? response.output : '')
            const decision = local.decision || (answers.length ? 'answer' : '')
            return {
                type: 'question',
                partKey,
                callId: id,
                runId: sessionId,
                questions: questionList(action.arguments),
                answers,
                decision,
                status: decision ? 'answered' : 'pending',
                expired: Boolean(expired[id]),
            }
        }
        const decision = local.decision || permissionToDecision(permissions.get(id)) || ''
        return {
            type: 'confirmation',
            partKey,
            callId: id,
            runId: sessionId,
            tool: String(action.toolName || ''),
            command: commandOf(action.arguments),
            reason: String(action.prompt || ''),
            decision,
            status: decision ? (decision === 'always' ? 'session_allowed' : decision === 'reject' ? 'denied' : 'allowed_once') : 'awaiting',
            expired: Boolean(expired[id]),
        }
    }
    for (const message of messages || []) {
        const visible = !isHiddenMessage(message)
        const group = ignoreSubagent ? null : groupOf.get(String((message && message.id) || ''))
        blocksOf(message).forEach((block, index) => {
            if (!PLAIN(block) || !block.type) return
            const partKey = `${(message && message.id) || 'msg'}:${index}`
            if (!visible) return
            if (group) {
                if (!group.anchorKey) group.anchorKey = parts.length ? parts[parts.length - 1].partKey : ''
                if (block.type === 'actionRequired') {
                    const action = actionOf(block)
                    if (action) tailParts.push(actionPart(action, partKey))
                }
                return
            }
            if (block.type === 'text') {
                const content = String(block.text || '')
                if (content.trim()) parts.push({ type: 'markdown', partKey, content, isStreaming: streaming })
                return
            }
            if (block.type === 'thinking') {
                const content = String(block.thinking || '')
                if (content.trim()) parts.push({ type: 'thinking', partKey, content, isStreaming: streaming })
                return
            }
            if (block.type === 'toolRequest') {
                const id = String(block.id || '')
                if (id) requestIds.add(id)
                const call = block.toolCall || {}
                const value = call.status === 'success' && PLAIN(call.value) ? call.value : {}
                const name = String(value.name || '')
                const args = PLAIN(value.arguments) ? value.arguments : {}
                const response = responses.get(id)
                const action = actions.get(id)
                if (name === 'todo_write') {
                    const todos = todoRows(args, response ? response.output : '')
                    parts.push({ type: 'todo', partKey, callId: id, title: '任务待办清单', todos })
                    return
                }
                const meta = response ? response.meta : null
                if (name === 'file_write' && meta && Array.isArray(meta.hunks) && response.status === 'completed') {
                    parts.push({ type: 'diff', partKey, callId: id, filePath: String(args.path || meta.path || ''), additions: Number(meta.additions || 0), deletions: Number(meta.deletions || 0), hunks: meta.hunks, truncated: Boolean(meta.truncated), status: response.status, meta })
                    return
                }
                const suppressed = Boolean(action) && (String(action.toolName || '') === ASK_TOOL || !response)
                const status = call.status === 'error' ? 'failed' : response ? response.status : suppressed ? 'pending' : streaming ? 'running' : 'terminated'
                if (name === 'agent_call') agentCalls.push({ partKey, callId: String((meta && meta.callId) || ''), agentId: String((meta && meta.agentId) || args.agent_id || '') })
                parts.push({
                    type: 'tool',
                    partKey,
                    callId: id,
                    toolName: name || (call.status === 'error' ? 'tool_request' : ''),
                    arguments: JSON.stringify(args),
                    output: response ? response.output : String(call.error || ''),
                    status,
                    isError: call.status === 'error' || Boolean(response && response.isError),
                    rejected: permissionToDecision(permissions.get(id)) === 'reject',
                    meta,
                    suppressed,
                })
                return
            }
            if (block.type === 'toolResponse') {
                const id = String(block.id || '')
                if (!id || requestIds.has(id)) return
                const response = responses.get(id) || { status: 'completed', output: '' }
                parts.push({ type: 'tool', partKey, callId: id, toolName: 'tool_result', arguments: '{}', output: response.output, status: response.status })
                return
            }
            if (block.type === 'actionRequired') {
                const action = actionOf(block)
                if (!action) return
                parts.push(actionPart(action, partKey))
                return
            }
            if (block.type === 'error') {
                parts.push({ type: 'error', partKey, content: String(block.message || '') })
                return
            }
            if (block.type === 'systemNotification') {
                if (block.msg === 'context_compaction') {
                    const data = PLAIN(block.data) ? block.data : {}
                    parts.push({ type: 'compaction', partKey, ...data })
                    return
                }
                parts.push({ type: 'unsupported', partKey, originalType: 'systemNotification', title: '系统提示', message: String(block.msg || ''), original: block })
                return
            }
            if (block.type === 'image') {
                parts.push({ type: 'unsupported', partKey, originalType: 'image', title: '图片内容', message: `模型返回图片（${String(block.mimeType || '未知类型')}），当前视图暂不展示。`, original: block })
                return
            }
            if (block.type === 'document') {
                parts.push({ type: 'unsupported', partKey, originalType: 'document', title: '文档内容', message: `${String(block.name || '文档')}（${String(block.mimeType || '未知类型')}）`, original: block })
                return
            }
            if (block.type === 'redactedThinking') {
                parts.push({ type: 'unsupported', partKey, originalType: 'redactedThinking', title: '已隐藏的思考内容', message: '模型返回了不可见的思考块，已省略展示。', original: block })
                return
            }
            parts.push({ type: 'unsupported', partKey, originalType: block.type, original: block })
        })
    }
    const merged = mergeAdjacent(parts)
    const withCards = ignoreSubagent ? merged : insertAgentCards(merged, groups, agentCalls, { streaming, responses, tailId: tailVisibleId(messages) })
    const finalParts = withCards.concat(tailParts)
    const lastIndex = finalParts.length - 1
    return finalParts.map((part, index) => {
        if (!part || (part.type !== 'thinking' && part.type !== 'markdown')) return part
        const activeTail = Boolean(streaming) && index === lastIndex
        return part.isStreaming === activeTail ? part : { ...part, isStreaming: activeTail }
    })
}