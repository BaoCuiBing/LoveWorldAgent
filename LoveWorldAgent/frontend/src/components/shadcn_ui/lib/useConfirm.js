import { reactive } from 'vue'
// 全局确认框状态：把原确认框的回调式用法改成 Promise
export const confirmState = reactive({
    open: false,
    title: '',
    description: '',
    confirmText: '确定',
    cancelText: '取消',
    destructive: false,
    resolve: null,
})
export const confirm = (options = {}) => new Promise((resolve) => {
    // 若上一次确认框仍未关闭，先按取消处理，避免 Promise 悬挂
    if (confirmState.resolve) confirmState.resolve(false)
    confirmState.open = true
    confirmState.title = options.title || '提示'
    confirmState.description = options.description || options.message || ''
    confirmState.confirmText = options.confirmText || '确定'
    confirmState.cancelText = options.cancelText || '取消'
    confirmState.destructive = Boolean(options.destructive)
    confirmState.resolve = resolve
})
export const resolveConfirm = (result) => {
    const resolve = confirmState.resolve
    confirmState.open = false
    confirmState.resolve = null
    if (resolve) resolve(result)
}
export const useConfirm = () => ({ confirm, state: confirmState, resolveConfirm })
// 组件入口：App.vue 挂载一次即可全局生效
export { default as ConfirmHost } from './ConfirmHost.vue'
