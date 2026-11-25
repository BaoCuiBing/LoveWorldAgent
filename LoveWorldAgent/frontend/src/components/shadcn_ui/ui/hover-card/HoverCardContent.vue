<script setup>
import { HoverCardContent, HoverCardPortal } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    side: { type: String, default: 'bottom' },
    align: { type: String, default: 'center' },
    sideOffset: { type: Number, default: 6 },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <HoverCardPortal>
        <HoverCardContent
            :side="props.side"
            :align="props.align"
            :side-offset="props.sideOffset"
            :class="cn('z-50 w-64 rounded-md border border-border bg-popover p-3 text-xs text-popover-foreground shadow-lg outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0', props.class)"
        >
            <slot />
        </HoverCardContent>
    </HoverCardPortal>
</template>
