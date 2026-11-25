<script setup>
import { SelectContent, SelectPortal, SelectViewport } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    position: { type: String, default: 'popper' },
    side: { type: String, default: 'bottom' },
    sideOffset: { type: Number, default: 4 },
    align: { type: String, default: 'start' },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <SelectPortal>
        <SelectContent
            :position="props.position"
            :side="props.side"
            :side-offset="props.sideOffset"
            :align="props.align"
            :class="cn('relative z-50 max-h-72 min-w-32 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95', props.class)"
        >
            <SelectViewport class="p-1">
                <slot />
            </SelectViewport>
        </SelectContent>
    </SelectPortal>
</template>
