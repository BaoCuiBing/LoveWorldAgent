/** 本地存储封装：统一前缀，读写失败时回退默认值 */
const PREFIX = 'lwa:'
const keyOf = (key) => `${PREFIX}${key}`
export const getString = (key, fallback = '') => {
    try {
        const value = localStorage.getItem(keyOf(key))
        return value === null ? fallback : value
    } catch {
        return fallback
    }
}
export const setString = (key, value) => {
    try {
        localStorage.setItem(keyOf(key), String(value))
    } catch {
        return
    }
}
export const getNumber = (key, fallback = 0) => {
    const raw = getString(key, '')
    if (raw === '') return fallback
    const value = Number(raw)
    return Number.isFinite(value) ? value : fallback
}
export const setNumber = (key, value) => setString(key, value)
