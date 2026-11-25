<script setup>
import { computed, inject } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
const command = inject('command', null)
// 仅在输入了关键字且没有任何条目命中时提示为空
const visible = computed(() => {
    if (!command) return false
    if (!command.query.value.trim()) return false
    return !command.items.value.some((item) => item.match())
})
</script>

<template>
    <div v-if="visible" :class="cn('py-4 text-center text-xs text-muted-foreground', props.class)">
        <slot>没有匹配结果</slot>
    </div>
</template>
