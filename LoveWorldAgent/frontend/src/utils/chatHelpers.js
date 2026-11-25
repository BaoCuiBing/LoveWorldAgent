import { blocksOf, isHiddenMessage, isPromptMessage, textOfMessage } from './chatContract' // 导入消息结构判断工具
import { pendingActions } from './chatEventReducer' // 导入未决审批提取函数
export const SIDE_KEY = 'ui.chat.sideWidth' // 左侧栏宽度本地存储键
export const RIGHT_KEY = 'ui.chat.rightWidth' // 右侧栏宽度本地存储键
export const API_BASE = location.protocol === 'file:' ? 'http://127.0.0.1:8866' : '' // 桌面端直连后端否则走代理
export const SIDE_MIN = 200 // 左侧栏最小像素宽度
export const SIDE_MAX = 420 // 左侧栏最大像素宽度
export const SIDE_DEFAULT = 264 // 左侧栏默认像素宽度
export const RIGHT_MIN = 420 // 右侧栏最小像素宽度
export const RIGHT_MAX = 1000 // 右侧栏最大像素宽度
export const RIGHT_RATIO = 0.44 // 首次未记忆时右栏占内容区比例
export const MAIN_MIN = 480 // 中间主对话区保底最小宽度
export const BAR_WIDTH = 12 // 单条分隔条占用宽度
export const PAGE_PADDING = 24 // 对话页左右总内边距
export const RUNNING_STATUS = ['queued', 'streaming', 'paused', 'waiting_approval', 'waiting_question'] // 视为运行中的轮次状态集合
export const NOTIFY_TEXT = { // 任务状态提示弹窗文案映射
    win: { title: '任务已完成', body: 'Agent 已完成本轮任务', type: 'success' }, // 任务正常完成文案
    wait: { title: '需要你的操作', body: 'Agent 正在等待你的确认', type: 'warning' }, // 等待用户审批或提问文案
    error: { title: '任务异常', body: '本轮任务出现错误', type: 'error' }, // 任务执行出错文案
}
const VALID_RIGHT_TABS = ['todo', 'build', 'file', 'terminal'] // 允许恢复的右侧栏有效页签
const STALE_LABEL_MAP = { failed: '任务异常', interrupted: '任务中断', completed: '任务完成' } // 过期运行状态中文标签
export const nowTime = () => new Date().toTimeString().slice(0, 5) // 获取当前时间 HH:MM 字符串
export const clampSide = (value) => Math.min(SIDE_MAX, Math.max(SIDE_MIN, value)) // 限制左侧栏宽度在合法范围
export const clampRight = (value) => Math.min(RIGHT_MAX, Math.max(RIGHT_MIN, value)) // 限制右侧栏宽度在合法范围
export const calcDefaultRightWidth = (innerWidth) => { // 根据窗口宽度计算默认右栏宽度
    const content = Math.max(0, innerWidth - PAGE_PADDING) // 扣除页面边距后的可用内容宽
    const cap = Math.max(RIGHT_MIN, Math.min(RIGHT_MAX, content - SIDE_DEFAULT - MAIN_MIN - BAR_WIDTH * 2)) // 为中栏和左栏预留空间上限
    return Math.min(cap, Math.max(RIGHT_MIN, Math.round(content * RIGHT_RATIO))) // 按比例计算并夹紧默认右栏宽
}
export const calcBarsWidth = (rightOpen) => BAR_WIDTH * (rightOpen ? 2 : 1) // 计算可见分隔条总宽度
export const calcMaxSideWidth = (containerW, rightOpen, rightW) => Math.max(SIDE_MIN, Math.min(SIDE_MAX, containerW - calcBarsWidth(rightOpen) - MAIN_MIN - (rightOpen ? rightW : 0))) // 计算左栏可拖拽最大宽度
export const calcMaxRightWidth = (containerW, rightOpen, sideW) => Math.max(RIGHT_MIN, Math.min(RIGHT_MAX, containerW - calcBarsWidth(rightOpen) - MAIN_MIN - sideW)) // 计算右栏可拖拽最大宽度
export const normalizeSavedTab = (rawTab) => { // 归一化布局记录中的右侧页签名
    const mapped = rawTab === 'browser' ? 'build' : rawTab // 兼容旧版 browser 页签名称
    return VALID_RIGHT_TABS.includes(mapped) ? mapped : '' // 非法页签返回空字符串
}
export const parseJson = (value) => { // 安全解析 JSON 对象字符串
    if (value && typeof value === 'object') return value // 已是对象则直接复用
    try { return JSON.parse(String(value || '')) || {} } catch { return {} } // 解析失败回退空对象
}
export const baseName = (value) => String(value || '').replace(/[\\/]+$/, '').split(/[\\/]/).pop() || '' // 提取路径末段文件名
export const hostName = (url) => { try { return new URL(String(url)).host || String(url) } catch { return String(url || '') } } // 提取链接主机名失败则回退原串
export const relOfPath = (path, projectPath) => { // 将绝对路径转为相对当前项目的斜杠路径
    let rel = String(path || '').trim().replace(/\\/g, '/') // 统一转为正斜杠并裁剪空白
    if (!rel) return '' // 空路径直接返回
    const base = String(projectPath || '').replace(/[\\/]+$/, '').replace(/\\/g, '/') // 规范项目根目录前缀
    if (base && rel.startsWith(`${base}/`)) rel = rel.slice(base.length + 1) // 剥离项目根前缀
    return rel.replace(/^\/+/, '') // 去掉开头多余斜杠
}
export const extractContextRefs = (content) => { // 提取文本中的 #File 与 #Folder 引用路径
    const refs = [] // 存储匹配到的路径列表
    const refRe = /#(?:File|Folder):([^\s#]+)/g // 引用标记正则
    let matched // 单次正则匹配结果
    while ((matched = refRe.exec(String(content || ''))) !== null) refs.push(matched[1]) // 收集捕获组路径
    return refs // 返回全部引用路径
}
export const computeBaselineContext = ({ messages = [], ruleCatalog = [], skillCatalog = [], sessionContextData = {}, mcpServersActive = [], projectPath = '', prevOthers = [] }) => { // 基于历史消息与目录生成基础上下文清单
    const nextRules = [] // 待填充的生效规则列表
    const pushRule = (item) => { // 去重写入规则项
        if (!item || !item.name || nextRules.some((r) => r.name === item.name)) return // 忽略空项或重复规则
        nextRules.push({ name: item.name, trigger: item.trigger || '', preview: item.preview || '' }) // 记录规则展示元数据
    }
    ruleCatalog.filter((item) => item && item.trigger === 'always_on').forEach(pushRule) // 默认注入常驻规则
    const nextSkills = [] // 待填充的技能列表
    const pushSkill = (item) => { // 去重写入技能项
        if (!item || !item.name || nextSkills.some((s) => s.name === item.name)) return // 忽略空项或重复技能
        nextSkills.push({ name: item.name, description: item.description || '' }) // 记录技能说明
    }
    skillCatalog.forEach(pushSkill) // 注入可用技能目录
    const nextFiles = [] // 待填充的上下文文件列表
    const pushFile = (rawPath) => { // 转为相对路径并去重记录
        const rel = relOfPath(rawPath, projectPath) // 转换相对项目路径
        if (rel && !nextFiles.includes(rel)) nextFiles.push(rel) // 去重追加文件
    }
    const nextOthers = [] // 待填充的扩展上下文条目
    const pushOther = (label, kind, path = '') => { // 去重写入其他上下文来源
        const text = String(label || '').trim() // 清洗展示标签
        if (!text || nextOthers.some((o) => o.label === text && o.kind === kind)) return // 同名同类型去重
        nextOthers.push({ label: text, kind, path: String(path || '') }) // 追加上下文条
    }
    const summaryText = String((sessionContextData.summary && sessionContextData.summary.summary) || '').trim() // 提取会话历史摘要
    if (summaryText) pushOther(summaryText.slice(0, 60), '历史摘要') // 截断并加入摘要条
    for (const fact of sessionContextData.facts || []) pushOther(fact, '关键事实') // 加入沉淀关键事实
    for (const srv of mcpServersActive) pushOther(srv.name, `MCP (${srv.tools}工具)`) // 加入已连接 MCP 服务
    for (const message of messages) { // 遍历历史消息抽取关联资源
        if (!message) continue // 跳过空消息
        if (isPromptMessage(message)) extractContextRefs(textOfMessage(message)).forEach(pushFile) // 抽取用户提示中显式引用文件
        for (const block of blocksOf(message)) { // 扫描消息内容块中的工具调用
            if (!block || block.type !== 'toolRequest') continue // 仅关心工具请求块
            const req = (block.toolCall && block.toolCall.value) || block.toolCall || {} // 取出工具调用结构
            const tname = String(req.name || '') // 工具名
            const args = parseJson(req.arguments) // 解析工具入参
            if (tname === 'rule_read' && args.name) pushRule(ruleCatalog.find((item) => item.name === args.name) || { name: args.name }) // 记录读取过的规则
            else if (tname === 'skill_read' && args.name) pushSkill(skillCatalog.find((item) => item.name === args.name) || { name: args.name }) // 记录读取过的技能
            else if (['file_read', 'file_write', 'file_delete'].includes(tname) && args.path) pushFile(args.path) // 记录读写删除的文件
            else if ((tname.startsWith('word_') || tname.startsWith('ppt_') || tname.startsWith('excel_')) && args.path) { // 记录 Office 文档工具操作的文件
                pushFile(args.path) // 加入文件列表
                pushOther(baseName(args.path), 'Office 文档', args.path) // 加入 Office 文档分类条目
            } else if ((tname === 'browser_navigate' || tname === 'browser_open') && args.url) pushOther(hostName(args.url), '联网页面', args.url) // 记录浏览器访问网址
        }
    }
    for (const existing of prevOthers || []) { // 保留本轮运行态产生的临时项
        if (['上传附件', '浏览器截图'].includes(existing.kind)) pushOther(existing.label, existing.kind, existing.path) // 回填上传附件与截图
    }
    return { rules: nextRules.slice(-50), skills: nextSkills.slice(-50), contextFiles: nextFiles.slice(-50), contextOthers: nextOthers.slice(0, 50) } // 返回截断上限后的四类上下文集合
}
export const extractTerminalTarget = (argsObj, metaObj, resultText, fallbackCwd) => { // 提取命令行工具对应的终端会话与工作目录
    const raw = String(resultText || '') // 转换原始工具输出文本
    const directSid = String(argsObj?.session_id || argsObj?.sessionId || metaObj?.sessionId || metaObj?.session_id || (/"session_?[Ii]d"\s*:\s*"([^"]+)"/.exec(raw) || [])[1] || '').trim() // 依次从入参、元数据和输出抽取终端 ID
    const directCwd = String(metaObj?.cwd || argsObj?.cwd || fallbackCwd || '') // 解析终端工作目录
    return { directSid, directCwd } // 返回终端标识与目录
}
export const pickTerminalSession = (list, directCwd) => { // 从活跃终端列表挑选最匹配的终端会话
    const rows = Array.isArray(list) ? list : [] // 确保输入为数组
    const alive = rows.filter((s) => s && s.alive) // 过滤仍存活的终端
    return alive.find((s) => directCwd && s.cwd === directCwd) || alive[alive.length - 1] || rows[rows.length - 1] || null // 优先同目录或最新终端
}
export const parseContextSize = (value) => { // 将 200k/1M 类上下文档位文本转为整数 token 上限
    const matched = /^(\d+(?:\.\d+)?)\s*([km])?$/i.exec(String(value || '').trim()) // 解析数值与单位后缀
    if (!matched) return 0 // 无法识别返回 0
    const size = Number(matched[1]) // 提取数值基数
    if (!Number.isFinite(size)) return 0 // 非有限数过滤
    const unit = String(matched[2] || '').toLowerCase() // 归一化单位字母
    if (unit === 'k') return Math.round(size * 1000) // 千单位换算
    if (unit === 'm') return Math.round(size * 1000000) // 百万单位换算
    return Math.round(size) // 无后缀按整数返回
}
export const buildChatHistory = (messages, excludeTurnKey = '') => { // 组装发送或压缩接口使用的纯文本历史数组
    const rows = messages || [] // 容错空消息列表
    const index = excludeTurnKey ? rows.findIndex((message) => String((message && message.id) || '') === String(excludeTurnKey)) : -1 // 定位需截断排除的轮次位置
    const list = index >= 0 ? rows.slice(0, index) : rows.slice() // 截取目标轮次之前的历史
    const out = [] // 结果数组
    for (const message of list) { // 遍历筛选用户与助手可见文本
        const text = textOfMessage(message).trim() // 提取消息纯文本内容
        if (!text) continue // 跳过空文本消息
        if (isPromptMessage(message)) out.push({ role: 'user', content: text }) // 写入用户角色条目
        else if (message.role === 'assistant' && !isHiddenMessage(message)) out.push({ role: 'assistant', content: text }) // 写入非隐藏助手条目
    }
    return out // 返回精简历史记录
}
export const providerUsageDetail = (usage) => { // 转换 Provider 级 usage 事件为统一统计结构
    const raw = (usage && usage.usage) || {} // 读取内嵌用量对象
    const input = Number(raw.input_tokens || 0) // 输入 token 数
    const output = Number(raw.output_tokens || 0) // 输出 token 数
    return { input, output, cache_read: Number(raw.cache_read_input_tokens || 0), cache_write: Number(raw.cache_write_input_tokens || 0), reasoning: 0, total: Number(raw.total_tokens || input + output || 0) } // 返回标准化用量对象
}
export const messageUsageDetail = (usage) => { // 转换 Message 级 usage 事件为统一统计结构
    const raw = usage || {} // 容错空对象
    const input = Number(raw.inputTokens || 0) // 输入 token 数
    const output = Number(raw.outputTokens || 0) // 输出 token 数
    return { input, output, cache_read: Number(raw.cacheReadTokens || 0), cache_write: Number(raw.cacheWriteTokens || 0), reasoning: 0, total: Number(raw.totalTokens || input + output || 0) } // 返回标准化用量对象
}
export const mergeUsageDetail = (prev, detail, accumulate = false) => { // 合并或覆写当前会话 token 统计
    if (!accumulate) return detail // 非累加模式直接使用新统计
    const base = prev || {} // 读取原累计对象
    return {
        input: Number(base.input || 0) + Number(detail.input || 0), // 累加输入消耗
        output: Number(base.output || 0) + Number(detail.output || 0), // 累加输出消耗
        cache_read: Number(base.cache_read || 0) + Number(detail.cache_read || 0), // 累加缓存命中
        cache_write: Number(base.cache_write || 0) + Number(detail.cache_write || 0), // 累加缓存写入
        reasoning: Number(base.reasoning || 0) + Number(detail.reasoning || 0), // 累加思考消耗
        total: Number(base.total || 0) + Number(detail.total || 0), // 累加总量
    }
}
export const parseTodoRows = (text) => { // 解析 todo_write 工具返回的待办事项列表
    try {
        const parsed = JSON.parse(String(text || '')) // 反序列化待办 JSON
        if (!Array.isArray(parsed)) return [] // 非数组视为空列表
        return parsed.filter((item) => item && typeof item === 'object').map((item) => ({ content: String(item.content || item.title || ''), status: String(item.status || 'pending') })).filter((item) => item.content) // 规范化待办字段并剔除空条目
    } catch { return [] } // 解析失败返回空数组
}
export const resolveStaleSessionTarget = (messages) => { // 根据历史消息推导卡死 running 会话的真实终态
    const pending = pendingActions(messages) // 检测是否存在未决审批或提问
    const errored = messages.some((message) => blocksOf(message).some((block) => block && block.type === 'error')) // 检测是否包含错误块
    const target = errored ? 'failed' : !messages.length || pending.length ? 'interrupted' : 'completed' // 依次判定异常、中断或完成
    return { target, label: STALE_LABEL_MAP[target] || target } // 返回目标状态码及中文提示
}
