export const TRIGGER_MODES = { '#': 'context', '/': 'slash' } // 输入触发符号对应面板模式
export const PATH_TRIGGER = /#(File|Folder):([^#\n]*)$/ // 匹配光标前路径引用前缀
export const IMAGE_EXT = /\.(png|jpe?g|webp|gif|bmp)$/i // 支持上传预览的图片后缀
export const CONTEXT_FALLBACK = ['200k', '600k', '1M'] // 默认上下文档位回退列表
export const THINKING_FALLBACK = [{ value: 'low', label: '低' }, { value: 'medium', label: '中' }, { value: 'high', label: '高' }] // 默认思考强度回退项
export const THINKING_LABEL_MAP = { off: '关闭', none: '关闭', disabled: '关闭', enabled: '开启', low: '低', medium: '中', high: '高', max: '极高' } // 思考档位中文映射
export const isImageFile = (file) => String(file?.type || '').startsWith('image/') || IMAGE_EXT.test(String(file?.name || '')) // 判断文件是否为图片
export const resolveTextareaEl = (instance) => { // 从组件实例解析原生输入框元素
    if (!instance) return null // 无实例返回空
    const el = instance.$el // 获取根 DOM 节点
    if (el && el.tagName === 'TEXTAREA') return el // 本身即文本域直接返回
    return instance.textarea || (el && el.querySelector ? el.querySelector('textarea') : null) || null // 查找内部文本域
}
export const resizeTextareaEl = (el) => { // 自动调节 2~6 行输入框高度
    if (!el) return // 缺少节点跳过
    const style = window.getComputedStyle(el) // 获取排版计算样式
    const line = parseFloat(style.lineHeight) || 22 // 计算单行高度
    const padding = (parseFloat(style.paddingTop) || 0) + (parseFloat(style.paddingBottom) || 0) // 累加垂直内边距
    el.style.height = 'auto' // 先重置高度以便重新测算
    el.style.height = `${Math.min(Math.max(el.scrollHeight, line * 2 + padding), line * 6 + padding)}px` // 限制在 2~6 行之间
}
export const computeInsertText = (text, start, end, value) => ({ // 计算选区插入后的文本与新光标位置
    text: `${text.slice(0, start)}${value}${text.slice(end)}`, // 拼接插入后的完整字符串
    pos: start + value.length, // 计算插入后的光标偏移
})
export const measureCaretCoords = (el, anchor, content) => { // 测量文本域光标相对容器的浮层坐标
    if (!el || !anchor) return null // 缺少测量元素直接返回
    const caret = typeof el.selectionStart === 'number' ? el.selectionStart : content.length // 读取当前光标下标
    const style = window.getComputedStyle(el) // 提取输入框当前样式
    const mirror = document.createElement('div') // 创建隐藏镜像节点模拟排版
    const copied = ['font-family', 'font-size', 'font-weight', 'line-height', 'letter-spacing', 'word-spacing', 'text-indent', 'text-transform', 'padding-top', 'padding-right', 'padding-bottom', 'padding-left', 'border-top-width', 'border-right-width', 'border-bottom-width', 'border-left-width', 'white-space', 'word-break', 'overflow-wrap', 'tab-size'] // 同步影响换行的样式键名
    copied.forEach((name) => mirror.style.setProperty(name, style.getPropertyValue(name))) // 复制排版计算样式
    mirror.style.position = 'absolute' // 脱离文档流避免抖动
    mirror.style.top = '-9999px' // 移出可视纵轴
    mirror.style.left = '-9999px' // 移出可视横轴
    mirror.style.visibility = 'hidden' // 保持视觉隐藏
    mirror.style.boxSizing = 'border-box' // 与源输入框盒模型一致
    mirror.style.width = `${el.offsetWidth}px` // 对齐容器像素宽度
    mirror.style.height = 'auto' // 高度随内容展开
    mirror.textContent = content.slice(0, caret) // 写入光标前字符
    const mark = document.createElement('span') // 创建尾随锚点标记
    mark.textContent = '\u200b' // 填入零宽字符精准定位
    mirror.appendChild(mark) // 插入标记节点
    document.body.appendChild(mirror) // 挂载到页面计算几何位置
    const caretLeft = mark.offsetLeft - el.scrollLeft // 扣除横向滚动得出左偏移
    const caretTop = mark.offsetTop - el.scrollTop // 扣除纵向滚动得出上偏移
    document.body.removeChild(mirror) // 立即销毁临时镜像节点
    const container = anchor.closest('.input-message') || anchor.parentElement // 定位参考外层容器
    const containerRect = container.getBoundingClientRect() // 获取外层容器矩形
    const areaRect = el.getBoundingClientRect() // 获取文本域矩形
    return { left: areaRect.left - containerRect.left + caretLeft, top: areaRect.top - containerRect.top + caretTop, maxLeft: Math.max(4, containerRect.width - 224) } // 返回限制边界所需的坐标数据
}
export const applyCaretCoords = (coords, ...targets) => { // 将算出的坐标应用到浮层锚点
    if (!coords) return // 无坐标数据时忽略
    targets.forEach((target) => { // 遍历所有待更新锚点
        if (!target) return // 忽略空锚点
        target.style.left = `${Math.min(Math.max(4, coords.left), coords.maxLeft)}px` // 夹紧左右边界防止菜单溢出
        target.style.top = `${Math.max(0, coords.top)}px` // 限制最小顶边距
    })
}
export const matchPathTrigger = (text, caret) => { // 检查光标前是否命中文件路径触发前缀
    const matched = PATH_TRIGGER.exec(text.slice(0, caret)) // 执行尾部正则匹配
    if (!matched) return null // 未匹配返回空
    return { kind: matched[1] === 'File' ? 'file' : 'dir', start: caret - matched[0].length, query: String(matched[2] || '').trim() } // 返回类型、起点与搜索词
}
export const filterPathItems = (list, kind, limit = 30) => (list || []).filter((item) => (kind === 'dir' ? item.type === 'dir' : item.type === 'file')).slice(0, limit) // 按类型过滤并截断路径候选项
export const computePathReplace = (text, start, trigger, rel) => { // 计算选中路径后替换输入区间的结果
    let end = start + trigger.length // 从触发前缀末尾开始扫描
    while (end < text.length && !/[\s#]/.test(text[end])) end += 1 // 跳过未输入完的路径片段
    const insert = `${trigger}${rel} ` // 拼接标准化引用并附带空格
    return { text: `${text.slice(0, start)}${insert}${text.slice(end)}`, pos: start + insert.length } // 返回更新后文本与光标位
}
export const modelKey = (model) => `${model.provider}|${model.name}` // 生成模型列表项唯一键
export const thinkingLabelOf = (modelOrValue, value) => { // 解析思考档位对应的中文展示标签
    const target = typeof modelOrValue === 'object' && modelOrValue ? value : modelOrValue // 兼容传模型对象或直接传值
    const options = typeof modelOrValue === 'object' && modelOrValue ? modelOrValue.thinkingOptions : null // 提取模型自带思考选项
    const hit = Array.isArray(options) ? options.find((item) => item && item.value === target) : null // 优先查找模型定义标签
    return (hit && hit.label) || THINKING_LABEL_MAP[target] || target || '' // 回退通用中文映射或原值
}
export const normalizeModelRow = (item, index) => { // 将后端模型项规整为前端渲染结构
    const contextOptions = Array.isArray(item.context_options) && item.context_options.length ? item.context_options : CONTEXT_FALLBACK // 提取可选上下文档位
    const rawThinking = Array.isArray(item.thinking_options) && item.thinking_options.length ? item.thinking_options : THINKING_FALLBACK // 提取原始思考选项列表
    const thinkingOptions = rawThinking.map((opt) => (typeof opt === 'string' ? { value: opt, label: THINKING_LABEL_MAP[opt] || opt } : { value: opt.value, label: opt.label || THINKING_LABEL_MAP[opt.value] || opt.value })) // 规范为值标签对象数组
    return {
        index, // 后端数组下标
        name: item.model, // 模型标识名
        provider: item.provider, // 服务商标识
        display: item.display || item.provider, // 服务商展示名
        disabled: item.enabled === false, // 是否禁用
        context: item.context || contextOptions[0] || '200k', // 当前选中上下文档位
        thinking: item.thinking || (thinkingOptions[0] && thinkingOptions[0].value) || 'low', // 当前选中思考强度
        contextOptions, // 该模型支持的上下文档位列表
        thinkingTitle: item.thinking_title || '思考强度', // 该模型思考参数标题
        thinkingOptions, // 该模型支持的思考强度选项列表
        vision: Boolean(item.vision), // 是否支持视觉输入
    }
}
export const revokeAttachmentUrls = (items) => { // 释放附件列表生成的本地 Blob 地址
    (items || []).forEach((item) => { if (item && item.url && item.url.startsWith('blob:')) URL.revokeObjectURL(item.url) }) // 逐个回收预览 URL
}
export const buildSendPayload = (items) => (items || []).map((item) => ({ name: item.name, path: item.path, isImage: item.isImage, size: item.size })) // 组装发送消息携带的附件载荷
