<script setup>
import { DropdownMenuContent, DropdownMenuPortal } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    side: { type: String, default: 'bottom' },
    align: { type: String, default: 'start' },
    sideOffset: { type: Number, default: 4 },
    alignOffset: { type: Number, default: 0 },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DropdownMenuPortal>
        <DropdownMenuContent
            :side="props.side"
            :align="props.align"
            :side-offset="props.sideOffset"
            :align-offset="props.alignOffset"
            :class="cn('z-50 min-w-32 overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95', props.class)"
        >
            <slot />
        </DropdownMenuContent>
    </DropdownMenuPortal>
</template>
