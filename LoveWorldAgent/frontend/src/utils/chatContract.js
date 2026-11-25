/** 对话事件契约：后端 /api/chat/stream 的 SSE data 直接是 goose AgentEvent，
 *  message 字段为 goose_provider_types::conversation::message::Message（content 为内容块数组）。
 *  这里只放纯数据判定，不依赖任何 UI 组件。 */
export const AGENT_EVENT_TYPES = ['message', 'usage', 'messageUsage', 'mcpNotification', 'historyReplaced']
export const BLOCK_TYPES = ['text', 'image', 'document', 'toolRequest', 'toolResponse', 'toolConfirmationRequest', 'actionRequired', 'thinking', 'redactedThinking', 'systemNotification', 'error']
export const ACTION_TYPES = ['toolConfirmation', 'elicitation', 'elicitationResponse', 'toolConfirmationResponse']
export const DECISIONS = ['once', 'always', 'reject', 'answer']
export const ASK_TOOL = 'ask_user'
const PLAIN = (value) => Boolean(value) && typeof value === 'object' && !Array.isArray(value)
export const blocksOf = (message) => (message && Array.isArray(message.content) ? message.content : [])
export const textOfMessage = (message) => blocksOf(message).filter((block) => block && block.type === 'text').map((block) => String(block.text || '')).join('')
/** userVisible 优先，兼容后端 snake_case 的 user_visible */
export const isHiddenMessage = (message) => {
    const meta = message && message.metadata
    if (!PLAIN(meta)) return false
    const visible = meta.userVisible ?? meta.user_visible
    return visible === false
}
export const isPromptMessage = (message) => {
    if (!PLAIN(message) || message.role !== 'user') return false
    if (isHiddenMessage(message)) return false
    const blocks = blocksOf(message)
    if (!blocks.some((block) => block && block.type === 'text' && String(block.text || '').trim())) return false
    return !blocks.some((block) => block && (block.type === 'toolResponse' || (block.type === 'actionRequired' && block.data && block.data.actionType === 'toolConfirmationResponse')))
}
/** created 是 Unix 秒，列表沿用原来的 HH:MM 展示 */
export const timeOfCreated = (created) => {
    const value = Number(created || 0)
    if (!Number.isFinite(value) || value <= 0) return ''
    const date = new Date(value * 1000)
    return Number.isFinite(date.getTime()) ? date.toTimeString().slice(0, 5) : ''
}
export const permissionToDecision = (permission) => {
    const value = String(permission || '').toLowerCase()
    if (value === 'allow_once') return 'once'
    if (value === 'always_allow') return 'always'
    if (value === 'cancel' || value === 'deny_once' || value === 'always_deny') return 'reject'
    return ''
}
export const isToolConfirmation = (block) => Boolean(block && block.type === 'actionRequired' && block.data && block.data.actionType === 'toolConfirmation')
export const actionOf = (block) => (isToolConfirmation(block) ? block.data : null)
export const normalizeAgentEvent = (raw) => {
    if (!PLAIN(raw) || typeof raw.type !== 'string') return null
    if (!AGENT_EVENT_TYPES.includes(raw.type)) return null
    return raw
}
