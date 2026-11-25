// 模型厂商图标映射：图片统一收在 model_icons/，同名图片同时留档于 src-tauri/static/imgs（随包分发）
import deepseek from './model_icons/deepseek.png'
import glm from './model_icons/glm.png'
import kimi from './model_icons/kimi.png'
import openpangu from './model_icons/openpangu.png'
import qwen from './model_icons/qwen.png'
const ICONS = { deepseek, glm, kimi, qwen, openpangu, huawei: openpangu }
export const providerIcon = (provider) => ICONS[String(provider || '').trim().toLowerCase()] || null
