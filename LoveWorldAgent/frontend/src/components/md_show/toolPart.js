/** 分片共享逻辑：状态判定、参数/输出解析、耗时与摘要。
 *  只面向新契约（status/meta/arguments/output/actionRequired），不做旧字段回退与旧文案前缀判定。 */
import { normalizeCodeLang } from '../../utils/codeLang.js'
const ROW_LIMIT = 6  // 折叠态最多 6 行
const LINE_LIMIT = 100  // 每行最多 100 字符
export const STATUS_LABEL = { running: '执行中', success: '已执行', failed: '执行失败', pending: '待确认', terminated: '已终止' }
const text = (value) => (value === null || value === undefined ? '' : String(value))
/** 统一状态：综合契约字段 status、isError 与旁路元数据 meta.ok / meta.exitCode */
export const resolveStatus = (part) => {
    const status = text(part && part.status).toLowerCase()
    if (status === 'running') return 'running'
    if (status === 'pending') return 'pending'
    if (status === 'terminated' || status === 'cancelled') return 'terminated'
    if (status === 'error' || status === 'failed') return 'failed'
    const meta = part && part.meta
    if (part && (part.isError || (meta && meta.ok === false) || (meta && typeof meta.exitCode === 'number' && meta.exitCode !== 0))) return 'failed'
    return 'success'
}
export const runningWithoutResult = (part) => {
    if (resolveStatus(part) !== 'running') return false
    const value = outputOf(part)
    return value === undefined || value === null || value === '' || (typeof value === 'string' && !value.trim())
}
export const argsOf = (part) => (part ? part.arguments : undefined)
export const outputOf = (part) => (part ? part.output : undefined)
export const toolNameOf = (part) => text(part && part.toolName)
export const parseArgs = (value) => {
    if (value === undefined || value === null || value === '') return null
    if (typeof value === 'object') return value
    try {
        return JSON.parse(value)
    } catch {
        return null
    }
}
export const formatValue = (value) => {
    if (value === undefined || value === null || value === '') return '(无)'
    if (typeof value === 'string') {
        try {
            return JSON.stringify(JSON.parse(value), null, 2)
        } catch {
            return value
        }
    }
    try {
        return JSON.stringify(value, null, 2)
    } catch {
        return String(value)
    }
}
export const codeHeight = (value) => { const lines = String(value === null || value === undefined ? '' : value).split('\n').length; return `${Math.min(360, Math.max(80, lines * 20 + 24))}px` }
export const codeLang = (value) => normalizeCodeLang(value)
export const formatDuration = (value) => {
    const ms = Number(value || 0)
    if (!Number.isFinite(ms) || ms <= 0) return ''
    if (ms < 1000) return `${Math.round(ms)}ms`
    if (ms < 60000) return `${(ms / 1000).toFixed(ms < 10000 ? 2 : 1)}s`
    return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`
}
export const firstLine = (value, limit = LINE_LIMIT) => {
    const line = text(value).trim().split('\n')[0] || ''
    if (!line) return ''
    return line.length > limit ? `${line.slice(0, limit)}…` : line
}
export const truncate = (value) => {
    const raw = text(value)
    const lines = raw.split('\n')
    const clipped = lines.slice(0, ROW_LIMIT).map((line) => (line.length > LINE_LIMIT ? `${line.slice(0, LINE_LIMIT)}…` : line))
    const shown = clipped.join('\n')
    return { shown, truncated: shown !== raw }
}
// 终端原始输出里的转义序列：OSC（…\a 或 …\e\\）、CSI（ESC [ … 最终字节）、双字符转义
const ANSI_PATTERN = /\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[@-Z\\-_]/g
const stripCarriage = (line) => {
    if (!line.includes('\r')) return line
    return line.split('\r').reduce((acc, seg) => (seg.length >= acc.length ? seg : seg + acc.slice(seg.length)), '')
}
const cleanAnsi = (value) => {
    const raw = text(value).replace(ANSI_PATTERN, '')
    return raw.split(/\r?\n/).map(stripCarriage).join('\n').replace(/\s+$/, '')
}
/** 从工具输出里取可读正文：terminal_* 返回 JSON（output/motd/text），其余按原文 */
export const resultOutput = (value) => {
    if (value === null || value === undefined || value === '') return ''
    if (typeof value === 'object') {
        const hit = value.output ?? value.motd ?? value.text
        return cleanAnsi(hit === undefined ? JSON.stringify(value, null, 2) : hit)
    }
    const raw = text(value)
    const parsed = parseArgs(raw)
    if (parsed && typeof parsed === 'object' && (parsed.output !== undefined || parsed.motd !== undefined || parsed.text !== undefined)) {
        return cleanAnsi(parsed.output ?? parsed.motd ?? parsed.text)
    }
    return cleanAnsi(raw)
}
const MARKDOWN_START = /^(#{1,6}\s|```|\|\s*:?-{3,}|-\s|\*\s|\*\*)/i
const isMarkdownResult = (value) => {
    const raw = text(value).replace(/\r\n?/g, '\n').trim()
    if (!raw || !raw.includes('\n')) return false
    if (raw.includes('```')) return true
    const lines = raw.split('\n')
    if (MARKDOWN_START.test(lines[0].trim())) return true
    return lines.some((line, index) => index > 0 && /^\|\s*:?-{3,}/.test(line.trim()))
}
export const langOf = (part) => {
    const name = toolNameOf(part)
    if (name === 'skill_read') return 'markdown'
    if (isMarkdownResult(outputOf(part))) return 'markdown'
    if (name.startsWith('rule_') || ['file_list', 'file_read', 'file_write', 'file_delete', 'command_run'].includes(name)) return 'json'
    return 'text'
}
export const toolKindOf = (part) => {
    const kind = text(part && part.toolKind)
    if (kind) return kind
    const name = toolNameOf(part)
    if (name.startsWith('browser_')) return 'browser'
    if (name.startsWith('word_')) return 'word'
    if (name.startsWith('computer_')) return 'computer'
    if (name.startsWith('ppt_')) return 'ppt'
    if (name.startsWith('excel_')) return 'excel'
    if (name.startsWith('mobile_')) return 'mobile'
    if (name.startsWith('mcp_')) return 'mcp'
    if (name.startsWith('terminal_')) return 'terminal'
    if (name === 'command_run') return 'command'
    if (name.startsWith('file_')) return 'file'
    if (name.startsWith('agent_')) return 'subagent'
    return 'other'
}
/** 工具卡平铺：仅过滤被审批卡抑制的工具分片 */
export const visibleParts = (parts) => (parts || []).filter((part) => part && !(part.type === 'tool' && part.suppressed))