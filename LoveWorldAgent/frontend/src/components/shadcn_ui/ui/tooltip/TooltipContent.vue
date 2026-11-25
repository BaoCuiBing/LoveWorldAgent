<script setup>
import { TooltipContent, TooltipPortal } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    side: { type: String, default: 'top' },
    align: { type: String, default: 'center' },
    sideOffset: { type: Number, default: 4 },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <TooltipPortal>
        <TooltipContent
            :side="props.side"
            :align="props.align"
            :side-offset="props.sideOffset"
            :class="cn('z-50 overflow-hidden rounded-md border border-border bg-popover px-2 py-1 text-[11px] leading-4 text-popover-foreground shadow-md data-[state=delayed-open]:animate-in data-[state=delayed-open]:fade-in-0 data-[state=delayed-open]:zoom-in-95', props.class)"
        >
            <slot />
        </TooltipContent>
    </TooltipPortal>
</template>
