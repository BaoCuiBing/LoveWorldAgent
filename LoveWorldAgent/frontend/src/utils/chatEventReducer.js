// 对话事件 reducer：消费后端 SSE 直出的 goose AgentEvent 并维护消息与状态列表
import { actionOf, blocksOf, isHiddenMessage, isPromptMessage, normalizeAgentEvent, textOfMessage, timeOfCreated } from './chatContract.js' // 导入消息契约读取函数
export const isCompactionMessage = (message) => PLAIN(message) && ((PLAIN(message.metadata) && PLAIN(message.metadata.operations) && PLAIN(message.metadata.operations.context) && Boolean(message.metadata.operations.context.compactionCard)) || blocksOf(message).some((item) => item && item.type === 'systemNotification' && item.msg === 'context_compaction')) // 判断是否为独立展示的上下文压缩助手消息
const MAX_MESSAGES = 4000 // 单会话本地保留的最大消息数上限
const PLAIN = (value) => Boolean(value) && typeof value === 'object' && !Array.isArray(value) // 判断是否为非空普通对象
const activeStatuses = ['queued', 'streaming', 'paused'] // 判定助手仍处于活跃生成中的状态
export const createInitialState = () => ({ // 生成会话初始状态快照
    sessionKey: '', // 当前激活会话 ID
    sessionEpoch: 0, // 历史加载代数戳
    messages: [], // goose 原始消息列表
    userMeta: {}, // 用户消息时间与附件附加信息
    decisions: {}, // 本地已提交的审批或问答结果映射
    expired: {}, // 已过期失效的审批动作映射
    active: { turnKey: '', status: 'idle', failure: null }, // 当前活跃轮次运行态
    usage: { used: 0, limit: 200000 }, // 会话 token 用量与上下文上限
    usageDetail: null, // 分项用量明细快照
    diagnostics: [], // 事件消费诊断日志队列
})
const diagnostic = (state, code, message, details = {}) => ({ ...state, diagnostics: [...state.diagnostics.slice(-49), { code, message, details, at: new Date().toISOString() }] }) // 追加一条诊断记录并保留最近 50 条
const usageDetailOfProvider = (usage) => { // 从 Provider 事件提取分项 token 统计
    const raw = PLAIN(usage) && PLAIN(usage.usage) ? usage.usage : {} // 读取内层用量对象
    const input = Number(raw.input_tokens || 0) // 输入 token 计数
    const output = Number(raw.output_tokens || 0) // 输出 token 计数
    return {
        input, // 输入消耗
        output, // 输出消耗
        cache_read: Number(raw.cache_read_input_tokens || 0), // 缓存读取命中数
        cache_write: Number(raw.cache_write_input_tokens || 0), // 缓存写入数
        reasoning: 0, // 思考字段占位
        total: Number(raw.total_tokens || input + output || 0), // 总消耗回退输入加输出
    }
}
const usageDetailOfMessage = (usage) => { // 从消息级用量事件提取分项 token 统计
    const raw = PLAIN(usage) ? usage : {} // 容错空统计对象
    const input = Number(raw.inputTokens || 0) // 输入 token 计数
    const output = Number(raw.outputTokens || 0) // 输出 token 计数
    return {
        input, // 输入消耗
        output, // 输出消耗
        cache_read: Number(raw.cacheReadTokens || 0), // 缓存读取命中数
        cache_write: Number(raw.cacheWriteTokens || 0), // 缓存写入数
        reasoning: 0, // 思考字段占位
        total: Number(raw.totalTokens || input + output || 0), // 总消耗回退输入加输出
    }
}
const sameBlock = (left, right) => { // 判断两个非文本内容块是否对应同一动作
    if (!PLAIN(left) || !PLAIN(right) || left.type !== right.type) return false // 类型不同直接判否
    if (left.type === 'toolRequest' || left.type === 'toolResponse') return left.id === right.id // 工具请求与响应按 callId 对齐
    if (left.type === 'actionRequired') return Boolean(left.data && right.data && left.data.actionType === right.data.actionType && left.data.id === right.data.id) // 审批交互块按类型及 ID 对齐
    if (left.type === 'systemNotification') return left.msg === 'context_compaction' && right.msg === 'context_compaction' // 同压缩卡片块原地更新
    return false // 其余类型不合并覆盖
}
const mergeMessage = (existing, incoming) => { // 合并同 ID 流式消息分片（文本拼接、工具按 ID 覆盖）
    const content = [...blocksOf(existing)] // 复制已有消息内容块数组
    for (const block of blocksOf(incoming)) { // 遍历新到达的内容块
        if (!PLAIN(block)) continue // 跳过非法块节点
        if (block.type === 'text' || block.type === 'thinking') { // 处理流式正文或思考增量块
            const last = content[content.length - 1] // 读取末尾已有块
            const field = block.type === 'text' ? 'text' : 'thinking' // 对应文本内容字段名
            if (last && last.type === block.type && typeof last[field] === 'string') content[content.length - 1] = { ...last, [field]: last[field] + String(block[field] || '') } // 同类型连续文本直接拼接
            else content.push(block) // 跨类型则新开独立块
            continue // 继续处理下一块
        }
        const index = content.findIndex((item) => sameBlock(item, block)) // 查找同 ID 的既有非文本块
        if (index >= 0) content[index] = block // 命中则就地覆盖刷新状态
        else content.push(block) // 未命中则顺序追加
    }
    return { ...existing, ...incoming, content } // 返回合并后的完整消息对象
}
const upsertMessage = (state, message) => { // 按消息 ID 插入或合并更新会话消息
    const messages = state.messages || [] // 读取现有消息列表
    const id = message.id ? String(message.id) : '' // 提取标准化消息 ID
    if (!id) return { ...state, messages: [...messages, message].slice(-MAX_MESSAGES) } // 无 ID 消息直接尾部追加并截断上限
    const index = messages.findIndex((item) => item && String(item.id || '') === id) // 检索同 ID 消息索引
    if (index < 0) return { ...state, messages: [...messages, message].slice(-MAX_MESSAGES) } // 新 ID 直接追加并截断上限
    const next = messages.slice() // 浅拷贝数组保证不可变更新
    next[index] = mergeMessage(next[index], message) // 合并同 ID 流式分片
    return { ...state, messages: next } // 返回更新后状态
}
export const reduceAgentEvent = (state, raw) => { // 消费单条 AgentEvent 并产生新会话状态
    const event = normalizeAgentEvent(raw) // 规范化原始 SSE 事件载荷
    if (!event) return { state: diagnostic(state, 'UNKNOWN_EVENT', '未知的 AgentEvent'), accepted: false, code: 'UNKNOWN_EVENT' } // 无法识别时记入诊断日志
    if (event.type === 'message') { // 助手或后端回推的用户消息事件
        const message = event.message // 提取内层消息载荷
        if (!PLAIN(message)) return { state: diagnostic(state, 'INVALID_MESSAGE', 'AgentEvent.message 不是对象'), accepted: false, code: 'INVALID_MESSAGE' } // 校验消息体为普通对象
        if (isHiddenMessage(message) && PLAIN(message.metadata) && PLAIN(message.metadata.operations) && PLAIN(message.metadata.operations.context) && message.metadata.operations.context.compacted) return { state, accepted: true, code: 'CONTEXT_COMPACTED' } // 仅隐藏通知不写列表；可见压缩卡片正常流式更新
        if (isPromptMessage(message) && message.id) { // 若后端回推真实用户消息 ID，则替换本地 local-* 临时键以便撤回删除命中库记录
            const messages = state.messages || [] // 读取当前消息列
            const newId = String(message.id) // 后端真实消息 ID
            const localIdx = messages.findIndex((item) => item && String(item.id || '').startsWith('local-') && textOfMessage(item) === textOfMessage(message)) // 定位未同步的同名本地用户消息
            if (localIdx >= 0) { // 命中则替换 ID 并迁移元数据
                const nextMessages = messages.slice() // 拷贝消息数组
                const oldId = String(nextMessages[localIdx].id) // 原本地临时 ID
                nextMessages[localIdx] = { ...nextMessages[localIdx], ...message, id: newId } // 同步后端真实 ID
                const meta = (state.userMeta && state.userMeta[oldId]) || {} // 读取原本地附件与展示时间
                const nextMeta = { ...state.userMeta, [newId]: meta } // 迁移元数据到真实 ID
                const active = state.active && state.active.turnKey === oldId ? { ...state.active, turnKey: newId } : state.active // 同步更新活跃轮次键
                return { state: { ...state, messages: nextMessages, userMeta: nextMeta, active }, accepted: true, code: 'MESSAGE_SYNCED', replacedTurnKey: { oldId, newId } } // 返回同步状态与新旧键
            }
        }
        return { state: upsertMessage(state, message), accepted: true, code: 'MESSAGE' } // 写入或合并到消息列表
    }
    if (event.type === 'usage') { // 服务商全局用量统计事件
        const detail = usageDetailOfProvider(event.usage) // 转换分项统计格式
        return { state: { ...state, usageDetail: detail, usage: { ...state.usage, used: detail.total || state.usage.used } }, accepted: true, code: 'USAGE' } // 更新实时 token 占用
    }
    if (event.type === 'messageUsage') { // 单条消息绑定用量事件
        const detail = usageDetailOfMessage(event.usage) // 转换分项统计格式
        const messageId = event.messageId ? String(event.messageId) : '' // 提取关联消息 ID
        let next = state // 初始化待更新状态
        if (messageId) { // 若指定消息 ID 则回写其元数据
            const index = state.messages.findIndex((item) => item && String(item.id || '') === messageId) // 定位目标消息索引
            if (index >= 0) { // 找到消息则补充 usage 元数据
                const messages = state.messages.slice() // 拷贝消息数组
                const target = messages[index] // 取出目标消息
                messages[index] = { ...target, metadata: { ...(target.metadata || {}), usage: event.usage } } // 合并用量字段至元数据
                next = { ...state, messages } // 更新临时状态
            }
        }
        return { state: { ...next, usageDetail: detail, usage: { ...state.usage, used: detail.total || next.usage.used } }, accepted: true, code: 'MESSAGE_USAGE' } // 同步更新汇总用量与状态
    }
    if (event.type === 'historyReplaced') { // 压缩或重整后整表替换历史消息事件
        const messages = Array.isArray(event.messages) ? event.messages.filter(PLAIN) : [] // 过滤合法对象消息
        return { state: { ...state, messages }, accepted: true, code: 'HISTORY_REPLACED' } // 全量替换当前历史
    }
    if (event.type === 'mcpNotification') return { state, accepted: true, code: 'MCP_NOTIFICATION' } // 上下文刷新或 MCP 通知事件不记错误诊断
    return { state: diagnostic(state, 'IGNORED_EVENT', `暂不处理的 AgentEvent：${event.type}`), accepted: false, code: 'IGNORED_EVENT' } // 其余事件记为忽略
}
export const appendUserMessage = (state, { id, text, time = '', created = 0, attachments = [] }) => { // 在会话末尾插入用户发送轮次
    const message = { // 构造用户消息标准结构
        id, // 轮次唯一键
        role: 'user', // 发送角色标识
        created: created || Math.floor(Date.now() / 1000), // 秒级时间戳
        content: [{ type: 'text', text: String(text || '') }], // 纯文本正文块
        metadata: { userVisible: true, agentVisible: true }, // 同时对用户与智能体可见
    }
    return {
        ...state, // 保留原有会话状态字段
        messages: [...state.messages, message].slice(-MAX_MESSAGES), // 追加消息并维持上限
        userMeta: { ...state.userMeta, [id]: { time, attachments } }, // 保存展示时间与上传附件
        active: { turnKey: id, status: 'streaming', failure: null }, // 激活本轮流式状态
    }
}
export const markActive = (state, turnKey, status, extra = {}) => ({ // 更新当前活跃轮次状态与失败原因
    ...state, // 保留其余会话字段
    active: { turnKey, status, failure: status === 'failed' ? extra.failure || state.active.failure : null }, // 仅 failed 保留异常详情
})
export const markDecision = (state, actionId, decision, answers = []) => ({ ...state, decisions: { ...state.decisions, [actionId]: { decision, answers } } }) // 记录指定审批项的决策结果
export const markExpired = (state, actionId) => ({ ...state, expired: { ...state.expired, [actionId]: true } }) // 标记指定审批项已过期失效
export const applyHistory = (state, messages) => ({ ...state, messages: (messages || []).filter(PLAIN) }) // 用拉取的历史消息覆盖列表
export const pendingActions = (messages, decisions = {}) => { // 检索尚未响应且未决策的工具审批或用户提问
    const answered = new Set() // 收集后端已有回应的动作 ID
    for (const message of messages || []) { // 遍历历史全部消息
        for (const block of blocksOf(message)) { // 扫描消息内容块
            if (block && block.type === 'actionRequired' && block.data && block.data.actionType === 'toolConfirmationResponse' && block.data.id) answered.add(String(block.data.id)) // 记录已存在回执的动作 ID
        }
    }
    const seen = new Set() // 防止重复收集相同动作
    const list = [] // 待决动作返回列表
    let turnKey = '' // 追踪动作所属的用户轮次键
    for (const message of messages || []) { // 再次扫描按轮次关联未决动作
        if (isPromptMessage(message)) turnKey = String(message.id || '') // 遇到用户提问刷新当前轮次键
        for (const block of blocksOf(message)) { // 检查待确认动作块
            const action = actionOf(block) // 提取标准化待确认信息
            if (!action || !action.id) continue // 非待决动作跳过
            const id = String(action.id) // 转为字符串动作 ID
            if (answered.has(id) || seen.has(id) || decisions[id]) continue // 跳过已回应、重复或已决策项
            seen.add(id) // 标记已收录避免重复
            list.push({ id, toolName: String(action.toolName || ''), arguments: action.arguments || {}, prompt: action.prompt || '', messageId: String(message.id || ''), turnKey }) // 追加待决条目
        }
    }
    return list // 返回全部未决动作集合
}
const usageOfMessages = (messages) => { // 累加同一轮次内所有消息携带的 token 消耗
    let total = 0 // 初始化轮次总消耗
    for (const message of messages || []) { // 遍历轮次关联消息
        const usage = message && message.metadata && message.metadata.usage // 提取元数据用量对象
        if (PLAIN(usage)) total += Number(usage.totalTokens || usage.total_tokens || 0) // 累加有效总 token
    }
    return total // 返回单轮累计耗用
}
const hasErrorBlock = (messages) => (messages || []).some((message) => blocksOf(message).some((block) => block && block.type === 'error')) // 判断消息集合内是否存在错误块
export const groupTurns = (state) => { // 按用户提问将扁平消息分组为前端渲染轮次结构
    const messages = state.messages || [] // 提取消息数组
    const userMeta = state.userMeta || {} // 读取用户附加元数据
    const active = state.active || {} // 读取当前流式状态
    const turns = [] // 分组结果数组
    let current = null // 当前正在归集的轮次对象
    for (const message of messages) { // 顺序扫描所有消息
        if (!PLAIN(message)) continue // 跳过非对象记录
        if (isPromptMessage(message)) { // 遇到新用户提问开启新轮次
            const id = String(message.id || `turn-${turns.length + 1}`) // 确定轮次唯一标识
            const meta = userMeta[id] || {} // 获取时间与附件缓存
            current = {
                turnKey: id, // 轮次键
                ordinal: turns.length + 1, // 轮次序号
                user: { id, content: textOfMessage(message), time: meta.time || timeOfCreated(message.created), attachments: meta.attachments || [] }, // 用户气泡渲染数据
                messages: [], // 关联助手后续消息队列
            }
            turns.push(current) // 收录新轮次
            continue // 处理下一条消息
        }
        if (isCompactionMessage(message)) { // 独立压缩助手消息自成一轮，贴合附图 1 视觉归属
            current = { turnKey: String(message.id || `compact-${turns.length + 1}`), ordinal: turns.length + 1, user: null, messages: [message], isCompaction: true } // 新建压缩轮次
            turns.push(current) // 收录压缩轮次
            continue // 处理下一条消息
        }
        if (!current) { // 首条非提问消息则创建兜底空提问轮次
            current = { turnKey: `turn-${turns.length + 1}`, ordinal: turns.length + 1, user: null, messages: [] } // 初始化无提问轮次
            turns.push(current) // 收录兜底轮次
        }
        current.messages.push(message) // 归入当前助手回复列表
    }
    const pending = pendingActions(messages, state.decisions || {}) // 统计所有未决审批动作
    return turns.map((turn) => { // 计算每一轮的综合状态与助手视口对象
        const waiting = pending.filter((item) => item.turnKey === turn.turnKey) // 筛选属于当前轮次的未决动作
        let status = 'completed' // 默认状态设为已完成
        if (waiting.length) status = waiting.some((item) => item.toolName === 'ask_user') ? 'waiting_question' : 'waiting_approval' // 有待决项时区分提问或审批状态
        else if (hasErrorBlock(turn.messages)) status = 'failed' // 无待决但含错误块判为失败
        if (active.turnKey && active.turnKey === turn.turnKey && active.status && active.status !== 'idle') status = active.status // 当前活跃流状态优先级最高
        return {
            turnKey: turn.turnKey, // 轮次键
            ordinal: turn.ordinal, // 轮次序号
            user: turn.user, // 用户消息对象
            isCompaction: Boolean(turn.isCompaction), // 是否为独立压缩消息轮次
            assistant: { // 助手回答汇总数据
                messages: turn.messages, // 助手下属消息列表
                status, // 综合状态码
                failure: active.turnKey === turn.turnKey && active.status === 'failed' ? active.failure : null, // 失败详情仅在异常态透出
                usage: usageOfMessages(turn.messages), // 本轮 token 合计
                decisions: state.decisions || {}, // 决策表引用
                expired: state.expired || {}, // 过期表引用
                sessionId: state.sessionKey || '', // 所属会话 ID
                isCompaction: Boolean(turn.isCompaction), // 透传压缩轮次标识
                active: activeStatuses.includes(status), // 是否处于流式运行中
            },
        }
    })
}
export const removeTurnMessages = (state, turnKey, { after = false } = {}) => { // 从本地消息数组中剔除指定轮次（可选连带后续全部轮次）
    const messages = state.messages || [] // 读取当前消息列表
    const index = messages.findIndex((item) => item && String(item.id || '') === String(turnKey)) // 定位目标轮次起始用户消息
    if (index < 0) return state // 未找到直接返回原状态
    let end = messages.length // 默许截断至数组末尾（用于回滚）
    if (!after) { // 仅删单轮时截到下一条用户提问或独立压缩消息为止
        const nextPrompt = messages.findIndex((item, position) => position > index && (isPromptMessage(item) || isCompactionMessage(item))) // 查找下一轮起点
        if (nextPrompt >= 0) end = nextPrompt // 限制删除终点位置
    }
    const next = [...messages.slice(0, index), ...messages.slice(end)] // 拼接保留前段与后段消息
    const active = state.active && state.active.turnKey === turnKey ? { turnKey: '', status: 'idle', failure: null } : state.active // 若删掉正在进行的轮次则重置活跃态
    return { ...state, messages: next, active } // 返回更新后的本地会话状态
}
