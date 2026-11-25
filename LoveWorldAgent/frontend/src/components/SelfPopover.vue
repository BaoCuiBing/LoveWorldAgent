<template>
    <Popover v-model:open="visible">
        <PopoverTrigger as-child>
            <button class="user-trigger" aria-haspopup="menu">
                <span class="user-avatar"><User :size="14" /></span>
                <span class="user-name">本地用户</span>
            </button>
        </PopoverTrigger>
        <PopoverContent side="top" align="start" :side-offset="8" class="w-[230px]">
            <div class="user-popover">
                <div class="menu-panel">
                    <button class="menu-row" @click="submenu = 'appearance'">
                        <Sparkles :size="16" />
                        <span class="menu-label">外观</span>
                        <span class="menu-value">{{ themeLabel }}</span>
                        <ChevronRight class="menu-arrow" :size="14" />
                    </button>
                    <button class="menu-row" @click="submenu = 'language'">
                        <MessageSquare :size="16" />
                        <span class="menu-label">语言</span>
                        <span class="menu-value">{{ localeLabel }}</span>
                        <ChevronRight class="menu-arrow" :size="14" />
                    </button>
                    <button class="menu-row" @click="onSettings">
                        <Settings :size="16" />
                        <span class="menu-label">设置</span>
                    </button>
                    <button class="menu-row" @click="onLogout">
                        <LogOut :size="16" />
                        <span class="menu-label">退出登录</span>
                    </button>
                </div>
                <div v-if="submenu" class="sub-panel">
                    <button class="sub-head" @click="submenu = ''">
                        <ChevronLeft :size="14" />
                        <span>{{ submenuTitle }}</span>
                    </button>
                    <button v-for="option in subOptions" :key="option.value" class="sub-row" :class="{ selected: option.value === currentValue }" @click="selectOption(option.value)">
                        <span>{{ option.label }}</span>
                        <Check v-if="option.value === currentValue" :size="14" />
                    </button>
                </div>
            </div>
        </PopoverContent>
    </Popover>
</template>

<script setup>
import { computed, ref } from 'vue'
import { Check, ChevronLeft, ChevronRight, LogOut, MessageSquare, Settings, Sparkles, User } from 'lucide-vue-next'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/shadcn_ui/ui/popover'
import { toast } from '@/components/shadcn_ui/lib/useToast'
import { saveTheme } from '../api/request_data'
import { applyTheme, currentTheme } from '../theme'
const emit = defineEmits(['settings'])
const THEME_OPTIONS = [{ value: 'dark', label: '深色' }, { value: 'light', label: '浅色' }, { value: 'system', label: '跟随系统' }]
const LOCALE_OPTIONS = [{ value: 'zh-CN', label: '简体中文' }, { value: 'en', label: 'English' }]
const visible = ref(false)
const submenu = ref('')
const theme = currentTheme
const locale = ref(localStorage.getItem('ui-locale') || 'zh-CN')
const themeLabel = computed(() => THEME_OPTIONS.find((item) => item.value === theme.value)?.label || '跟随系统')
const localeLabel = computed(() => (locale.value === 'en' ? 'English' : '简体中文'))
const submenuTitle = computed(() => (submenu.value === 'appearance' ? '外观' : '语言'))
const subOptions = computed(() => (submenu.value === 'appearance' ? THEME_OPTIONS : LOCALE_OPTIONS))
const currentValue = computed(() => (submenu.value === 'appearance' ? theme.value : locale.value))
const selectOption = async (value) => {
    if (submenu.value === 'appearance') {
        try {
            await saveTheme(value)
            applyTheme(value)
            theme.value = value
            toast.success('外观已更新')
        } catch (err) {
            toast.error(`保存外观失败：${err.message}`)
            return
        }
    } else {
        locale.value = value
        localStorage.setItem('ui-locale', value)
        toast.success('语言偏好已保存')
    }
    submenu.value = ''
    visible.value = false
}
const onSettings = () => {
    visible.value = false
    emit('settings')
}
const onLogout = () => {
    visible.value = false
    toast.info('本地模式无需登录')
}
</script>

<style scoped>
.user-trigger { display: flex; align-items: center; gap: 8px; width: 100%; padding: 6px 8px; border: none; border-radius: 6px; background: transparent; color: var(--foreground); cursor: pointer; }
.user-trigger:hover { background: var(--accent); }
.user-avatar { display: inline-flex; align-items: center; justify-content: center; width: 24px; height: 24px; border-radius: 50%; color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.user-name { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.user-popover { position: relative; }
.menu-panel { display: flex; flex-direction: column; gap: 2px; width: 100%; }
.menu-row { display: flex; align-items: center; gap: 8px; width: 100%; height: 34px; padding: 0 8px; border: none; border-radius: 6px; background: transparent; color: var(--foreground); font-size: 13px; cursor: pointer; }
.menu-row:hover { background: var(--accent); }
.menu-label { flex: 1; text-align: left; }
.menu-value { font-size: 12px; color: var(--muted-foreground); }
.menu-arrow { color: var(--muted-foreground); }
.sub-panel { position: absolute; left: calc(100% + 8px); top: 0; width: 210px; padding: 6px; border: 1px solid var(--border); border-radius: 8px; background: var(--popover); box-shadow: 0 4px 16px rgb(0 0 0 / 0.28); }
.sub-head { display: flex; align-items: center; gap: 6px; width: 100%; height: 30px; padding: 0 8px; border: none; border-radius: 6px; background: transparent; color: var(--muted-foreground); font-size: 12px; cursor: pointer; }
.sub-head:hover, .sub-row:hover { background: var(--accent); }
.sub-row { display: flex; align-items: center; justify-content: space-between; width: 100%; height: 32px; padding: 0 8px; border: none; border-radius: 6px; background: transparent; color: var(--foreground); font-size: 13px; cursor: pointer; }
.sub-row.selected { color: var(--primary); }
</style>
