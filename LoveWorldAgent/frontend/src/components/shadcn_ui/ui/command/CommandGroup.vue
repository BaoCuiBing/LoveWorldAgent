<script setup>
import { computed, inject } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    heading: { type: String, default: '' },
    class: { type: [String, Array, Object], default: '' },
})
const command = inject('command', null)
// 组内条目全部被过滤时整组隐藏
const visible = computed(() => {
    if (!command) return true
    return command.items.value.some((item) => item.group === props.heading && item.match())
})
</script>

<template>
    <div v-show="visible" :class="cn('py-0.5', props.class)">
        <div v-if="props.heading" class="px-2 py-1 text-[11px] font-medium text-muted-foreground">{{ props.heading }}</div>
        <slot />
    </div>
</template>
