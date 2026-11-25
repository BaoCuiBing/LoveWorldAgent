<script setup>
import { ContextMenuPortal, ContextMenuSubContent } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ContextMenuPortal>
        <ContextMenuSubContent
            :side-offset="2"
            :class="cn('z-50 min-w-32 overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-lg', props.class)"
        >
            <slot />
        </ContextMenuSubContent>
    </ContextMenuPortal>
</template>
