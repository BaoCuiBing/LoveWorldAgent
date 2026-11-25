<script setup>
import { PopoverContent, PopoverPortal } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    side: { type: String, default: 'bottom' },
    align: { type: String, default: 'center' },
    sideOffset: { type: Number, default: 6 },
    alignOffset: { type: Number, default: 0 },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <PopoverPortal>
        <PopoverContent
            :side="props.side"
            :align="props.align"
            :side-offset="props.sideOffset"
            :align-offset="props.alignOffset"
            :class="cn('z-50 w-64 rounded-md border border-border bg-popover p-3 text-xs text-popover-foreground shadow-lg outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95', props.class)"
        >
            <slot />
        </PopoverContent>
    </PopoverPortal>
</template>
