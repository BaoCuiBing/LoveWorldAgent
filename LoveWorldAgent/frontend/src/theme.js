// 主题控制：dark / light / system，html.dark 决定 Element Plus 暗色变量是否生效
import { ref } from 'vue'
import { getTheme, updateWindow } from './api/request_data'
export const THEME_STORAGE_KEY = 'ui-theme'
const media = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null
const normalize = (mode) => (['dark', 'light', 'system'].includes(mode) ? mode : 'dark')
export const currentTheme = ref(normalize(localStorage.getItem(THEME_STORAGE_KEY) || 'dark'))  // 全局共享主题，供组件状态同步
export const applyTheme = (mode) => {
    const value = normalize(mode)
    const dark = value === 'dark' || (value === 'system' && Boolean(media && media.matches))
    document.documentElement.classList.toggle('dark', dark)
    localStorage.setItem(THEME_STORAGE_KEY, value)
    currentTheme.value = value
    updateWindow(value).catch(() => {})  // 通知后端重绘窗口标题栏配色
    return value
}
export const initTheme = async () => {
    let mode = localStorage.getItem(THEME_STORAGE_KEY) || 'dark'
    try {
        const data = await getTheme()
        mode = data.theme || mode
    } catch {
        mode = localStorage.getItem(THEME_STORAGE_KEY) || 'dark'
    }
    return applyTheme(mode)
}
if (media && media.addEventListener) media.addEventListener('change', () => {
    if (localStorage.getItem(THEME_STORAGE_KEY) === 'system') applyTheme('system')
})
