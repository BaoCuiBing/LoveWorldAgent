import { reactive } from 'vue'
// 全局 toast 队列：以命令式 API 替代原消息提示
const state = reactive({ toasts: [] })
const DEFAULT_DURATION = 3000
let seed = 0
export const toasts = state.toasts
export const dismissToast = (id) => {
    const index = state.toasts.findIndex((item) => item.id === id)
    if (index > -1) state.toasts.splice(index, 1)
}
export const clearToasts = () => {
    state.toasts.splice(0)
}
const pushToast = (type, message, options = {}) => {
    const id = ++seed
    const duration = options.duration ?? DEFAULT_DURATION
    state.toasts.push({
        id,
        type,
        title: options.title || '',
        message: message === undefined || message === null ? '' : String(message),
        duration,
    })
    // duration 为 0 时保持常驻，需手动关闭
    if (duration > 0) setTimeout(() => dismissToast(id), duration)
    return id
}
// 与原消息提示调用习惯保持一致：toast.success('已保存')
export const toast = {
    success: (message, options) => pushToast('success', message, options),
    error: (message, options) => pushToast('error', message, options),
    warning: (message, options) => pushToast('warning', message, options),
    info: (message, options) => pushToast('info', message, options),
    message: (message, options) => pushToast('info', message, options),
    dismiss: dismissToast,
    clear: clearToasts,
}
export const useToast = () => ({ toast, toasts, dismiss: dismissToast, clear: clearToasts })
// 组件入口：App.vue 挂载一次即可全局生效
export { default as Toaster } from './Toaster.vue'
