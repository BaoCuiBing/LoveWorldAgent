<script setup>
import { provide, ref } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
// 关键字过滤：输入框写入 query，条目/Groups 读取 isMatch 判断是否展示
const query = ref('')
const items = ref([])
let seed = 0
const register = (match, group = '') => {
    const id = ++seed
    items.value.push({ id, match, group })
    return id
}
const unregister = (id) => {
    items.value = items.value.filter((item) => item.id !== id)
}
const isMatch = (value) => {
    const keyword = query.value.trim().toLowerCase()
    if (!keyword) return true
    return String(value ?? '').toLowerCase().includes(keyword)
}
provide('command', { query, items, register, unregister, isMatch })
</script>

<template>
    <div :class="cn('flex h-full w-full flex-col overflow-hidden rounded-md bg-popover text-popover-foreground', props.class)">
        <slot />
    </div>
</template>
