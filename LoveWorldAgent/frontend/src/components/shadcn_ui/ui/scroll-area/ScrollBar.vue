<script setup>
import { ScrollAreaScrollbar, ScrollAreaThumb } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    orientation: { type: String, default: 'vertical' },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ScrollAreaScrollbar
        :orientation="props.orientation"
        :class="
            cn(
                'flex touch-none select-none transition-colors',
                props.orientation === 'vertical' ? 'h-full w-1.5 border-l border-l-transparent p-px' : 'h-1.5 flex-col border-t border-t-transparent p-px',
                props.class,
            )
        "
    >
        <ScrollAreaThumb class="relative flex-1 rounded-full bg-border" />
    </ScrollAreaScrollbar>
</template>
